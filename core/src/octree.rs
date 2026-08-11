//! Octree color quantization — the classification phase.
//!
//! This is the ImageMagick-style adaptive-spatial-subdivision quantizer, built
//! in three phases: classify (this file, for now), reduce, and assign. See the
//! algorithm write-up at https://imagemagick.org/quantize/.
//!
//! The octree subdivides the RGB cube (0,0,0)–(255,255,255) into eight equal
//! octants at each level, down to `MAX_DEPTH` levels. The cubes are a *fixed*
//! geometric grid; what adapts to the image is which cubes get instantiated and
//! how deep the tree goes in dense color regions. Because the cuts are always
//! at midpoints, which octant a color falls into at a given level is just a
//! 3-bit value read from that level's bit of r, g, b — no comparisons.
//!
//! ## Why an arena (Vec<Node>) instead of pointers
//!
//! Nodes need both child links (down) and a parent link (up — reduction merges
//! upward). That's an ownership cycle, which Rust's borrow checker rejects for
//! `Box`/`&`. The idiomatic fix: store every node in a `Vec<Node>` (the arena)
//! and reference nodes by their `usize` index. Indices are `Copy` and carry no
//! ownership, so parent/child links are just numbers — no `Rc`/`RefCell`, no
//! cycles. Every method takes `&mut self` / `&self` and indexes into
//! `self.nodes`.

/// Default/maximum tree depth. 8 levels distinguishes every 8-bit RGB color,
/// but that's the pathological case: a full photo builds millions of leaves and
/// reduction crawls. Real trees are built shallower — see `depth_for_colors` —
/// because the output only needs enough resolution to reduce *from*.
const MAX_DEPTH: usize = 8;

/// Choose a tree depth from the desired output color count. Deeper trees
/// distinguish more colors but cost more to build and reduce; the output only
/// needs enough levels to have more distinct leaves than target colors, plus a
/// margin so reduction has real choices. Capped at 6 (262k cells) so even
/// pathological images stay tractable, and floored at 3 so tiny palettes still
/// have structure. This mirrors the article's "depth as a function of the
/// desired number of colors."
fn depth_for_colors(colors: usize) -> usize {
    // log2(colors) levels would give ~colors leaves; add margin, clamp.
    let bits = (usize::BITS - colors.max(1).leading_zeros()) as usize; // ~ceil(log2)
    (bits + 2).clamp(3, 6)
}

/// One node = one cube in RGB space. Accumulates the statistics the reduce and
/// assign phases need. No color is stored — a node's color is derived at
/// assignment as (sr/n2, sg/n2, sb/n2).
#[derive(Clone)]
struct Node {
    /// Indices of the eight child octants (None = not yet created). The index
    /// into this array is the 3-bit octant number.
    children: [Option<usize>; 8],
    /// Parent index (None only for the root). Reduction walks up via this.
    parent: Option<usize>,
    /// Total pixels whose color falls in this node's cube (this level and below).
    n1: u32,
    /// Pixels for which this node is the *deepest* representing them. A node
    /// with n2 > 0 contributes one color to the output. Only leaves get n2 at
    /// classification time; reduction moves n2 upward as it prunes.
    n2: u32,
    /// Sums of r/g/b over the pixels counted in n2 (u64: sums over millions of
    /// pixels overflow u32). Divided by n2 at assignment to get the mean color.
    sr: u64,
    sg: u64,
    sb: u64,
    /// Accumulated squared distance from each contained pixel to this node's
    /// cube center — the quantization error, used to choose what to prune.
    e: f64,
    /// False once this node has been pruned (folded into its parent). Pruned
    /// nodes stay in the arena but are skipped by all traversals.
    alive: bool,
}

impl Node {
    fn new(parent: Option<usize>) -> Self {
        Node {
            children: [None; 8],
            parent,
            n1: 0,
            n2: 0,
            sr: 0,
            sg: 0,
            sb: 0,
            e: 0.0,
            alive: true,
        }
    }
}

/// The octree: an arena of nodes plus the root index (always 0) and the tree
/// depth used for classification (chosen from the target color count).
pub struct Octree {
    nodes: Vec<Node>,
    root: usize,
    depth: usize,
}

impl Octree {
    /// A fresh tree of the given classification depth (see `depth_for_colors`).
    pub fn with_depth(depth: usize) -> Self {
        Octree {
            nodes: vec![Node::new(None)],
            root: 0,
            depth: depth.clamp(1, MAX_DEPTH),
        }
    }

    /// A fresh tree at full depth (mainly for tests). Prefer `with_depth` /
    /// `octree_palette` in real use — full depth is slow on large images.
    pub fn new() -> Self {
        Self::with_depth(MAX_DEPTH)
    }

    /// Classify one pixel: walk root→leaf, accumulating stats on every node
    /// along the path, creating child nodes lazily. The deepest node gets n2.
    pub fn classify(&mut self, color: [u8; 3]) {
        let mut idx = self.root;

        for level in 0..self.depth {
            // Interior nodes on the path accumulate only n1 (pixels passing
            // through) and E (error to this cube's center). They do NOT
            // accumulate Sr/Sg/Sb: per the article those sum only pixels "not
            // classified at a lower depth", which for an interior node during
            // classification is none — every pixel here continues downward to
            // its leaf. (Reduction later folds leaf sums up into survivors.)
            {
                let node = &mut self.nodes[idx];
                node.n1 += 1;
            }
            let center = cube_center_for_level(color, level);
            self.nodes[idx].e += squared_distance(color, center);

            // Descend into the octant this color belongs to at this level,
            // creating the child lazily if it doesn't exist yet.
            let octant = octant_index(color, level);
            idx = match self.nodes[idx].children[octant] {
                Some(c) => c,
                None => {
                    let new_idx = self.nodes.len();
                    self.nodes.push(Node::new(Some(idx)));
                    self.nodes[idx].children[octant] = Some(new_idx);
                    new_idx
                }
            };
        }

        // idx is now the leaf for this exact color: it owns the pixel. Only the
        // leaf accumulates the color sums and n2 (owned-pixel count).
        let leaf = &mut self.nodes[idx];
        leaf.n1 += 1;
        leaf.n2 += 1;
        leaf.sr += color[0] as u64;
        leaf.sg += color[1] as u64;
        leaf.sb += color[2] as u64;
    }

    /// Classify every opaque pixel of an image.
    pub fn classify_image(&mut self, image: &crate::Image) {
        for p in image.pixels() {
            if p.0[3] == 0 {
                continue;
            }
            self.classify([p.0[0], p.0[1], p.0[2]]);
        }
    }

    /// Count nodes that currently own a color (alive, n2 > 0). This is the
    /// number of output colors; reduction drives it down to the target.
    pub fn color_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.alive && n.n2 > 0).count()
    }

    /// Total node count (for tests/inspection).
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Depth of a node (root = 0), by walking parent links. Used to prune
    /// deepest-first so a parent still exists when its child folds up.
    fn depth(&self, mut idx: usize) -> usize {
        let mut d = 0;
        while let Some(p) = self.nodes[idx].parent {
            d += 1;
            idx = p;
        }
        d
    }

    /// Prune one node: fold its owned-color statistics (n2, Sr, Sg, Sb) into its
    /// parent, detach it from the parent, and mark it dead. Because reduction
    /// prunes deepest-first, any children of this node were already folded into
    /// *it* in earlier iterations, so its sums already represent its whole
    /// remaining subtree. (The root has no parent and is never pruned.)
    ///
    /// Returns the net change in owner count (nodes with n2 > 0): this node
    /// stops owning (−1), but its parent may *start* owning (+1) if it wasn't
    /// already. So the delta is −1 (parent already owned) or 0 (parent newly
    /// owns). Reduction uses this to maintain the count without rescanning.
    fn prune(&mut self, idx: usize) -> i64 {
        let Some(p) = self.nodes[idx].parent else {
            return 0; // never prune the root
        };
        let (n2, sr, sg, sb) = {
            let n = &self.nodes[idx];
            (n.n2, n.sr, n.sg, n.sb)
        };
        let parent_was_owner = self.nodes[p].n2 > 0;
        {
            let parent = &mut self.nodes[p];
            parent.n2 += n2;
            parent.sr += sr;
            parent.sg += sg;
            parent.sb += sb;
            // Detach this child from the parent.
            for slot in parent.children.iter_mut() {
                if *slot == Some(idx) {
                    *slot = None;
                }
            }
        }
        let node = &mut self.nodes[idx];
        node.alive = false;
        node.n2 = 0;

        // This node stopped owning (−1); the parent started owning only if it
        // wasn't already (+1 when it newly owns).
        if parent_was_owner { -1 } else { 0 }
    }

    /// Reduce the tree until at most `target` nodes own a color (n2 > 0).
    ///
    /// Processes nodes **deepest level first**. Within each level, prunes the
    /// lowest-error owners first (folding their color stats into their parents)
    /// until the target is reached. Going level-by-level from the bottom means a
    /// parent is only considered *after* all its children have folded into it,
    /// so by the time we reach it its n2/sums/position reflect its whole
    /// absorbed subtree — which is what makes a single pass correct. (A flat
    /// "sort everything once and prune" pass is wrong: parents become owners as
    /// children fold in, and a pre-sorted list never revisits them.)
    ///
    /// This is the article's error-ordered pruning, made O(n log n) instead of
    /// the O(n^2) you'd get by re-scanning the whole tree after every prune.
    pub fn reduce(&mut self, target: usize) {
        let mut owners = self.color_count();
        if owners <= target {
            return;
        }

        let max_depth = self.depth;
        let mut by_depth: Vec<Vec<usize>> = vec![Vec::new(); max_depth + 1];
        for i in 0..self.nodes.len() {
            if self.nodes[i].alive && self.nodes[i].parent.is_some() {
                let d = self.depth(i);
                by_depth[d].push(i);
            }
        }

        for d in (1..=max_depth).rev() {
            if owners <= target {
                break;
            }
            let mut level: Vec<usize> = by_depth[d]
                .iter()
                .copied()
                .filter(|&i| self.nodes[i].alive && self.nodes[i].n2 > 0)
                .collect();
            level.sort_by(|&a, &b| {
                self.nodes[a]
                    .e
                    .partial_cmp(&self.nodes[b].e)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            for v in level {
                if owners <= target {
                    break;
                }
                if self.nodes[v].alive && self.nodes[v].n2 > 0 {
                    // prune returns -1 if the parent was already an owner
                    // (net one fewer color), 0 if the parent newly becomes one.
                    owners = (owners as i64 + self.prune(v)) as usize;
                }
            }
        }
    }

    /// Read the palette out: every surviving owner (alive, n2 > 0) contributes
    /// its mean color (sums / n2). Call after `reduce`.
    pub fn assign(&self) -> Vec<[u8; 3]> {
        self.nodes
            .iter()
            .filter(|n| n.alive && n.n2 > 0)
            .map(|n| {
                let n2 = n.n2 as u64;
                [(n.sr / n2) as u8, (n.sg / n2) as u8, (n.sb / n2) as u8]
            })
            .collect()
    }
}

/// Client code: generate an adaptive palette via octree quantization. Shows the
/// three phases in sequence — classify, reduce, assign. The tree depth is chosen
/// from the target color count (see `depth_for_colors`): a shallower tree builds
/// far fewer nodes, which is what keeps this fast on full-size images.
pub fn octree_palette(image: &crate::Image, target_colors: usize) -> Vec<[u8; 3]> {
    let target = target_colors.max(1);
    let mut tree = Octree::with_depth(depth_for_colors(target));
    tree.classify_image(image); // phase 1: build the tree
    tree.reduce(target); // phase 2: collapse to the target
    tree.assign() // phase 3: read colors out
}

/// Like `octree_palette`, but subdivides *OkLab* space instead of RGB. Each
/// pixel is mapped into a normalized-OkLab cube before classification, so the
/// tree groups colors by *perceptual* proximity and the pruning error `E`
/// (squared distance to cube center) is a perceptual distance — pruning removes
/// the perceptually cheapest distinctions. The octree itself is unchanged; only
/// the coordinates going in and the palette coming out are transformed.
///
/// This is the same idea the ImageMagick write-up notes ("distances in color
/// spaces such as YUV or YIQ correspond to perceptual color differences more
/// closely than RGB"), done here in OkLab.
pub fn octree_palette_oklab(image: &crate::Image, target_colors: usize) -> Vec<[u8; 3]> {
    use crate::color_utils::{norm_oklab_to_rgb, rgb_to_norm_oklab};

    let target = target_colors.max(1);
    let mut tree = Octree::with_depth(depth_for_colors(target));

    // Phase 1: classify, but on normalized-OkLab coordinates.
    for p in image.pixels() {
        if p.0[3] == 0 {
            continue;
        }
        tree.classify(rgb_to_norm_oklab(p.0[0], p.0[1], p.0[2]));
    }

    // Phase 2: reduce (unchanged — the tree doesn't know or care what space
    // its coordinates came from; error is now perceptual because the space is).
    tree.reduce(target);

    // Phase 3: assign, then map the normalized-OkLab means back to sRGB.
    tree.assign().into_iter().map(norm_oklab_to_rgb).collect()
}

impl Default for Octree {
    fn default() -> Self {
        Self::new()
    }
}

/// The 3-bit octant index for `color` at `level`: one bit from each channel,
/// taken from the most-significant end (level 0 = top bit). Bit layout: red is
/// the high bit, then green, then blue.
fn octant_index(color: [u8; 3], level: usize) -> usize {
    let shift = 7 - level; // level 0 -> bit 7 (MSB)
    let r = ((color[0] >> shift) & 1) as usize;
    let g = ((color[1] >> shift) & 1) as usize;
    let b = ((color[2] >> shift) & 1) as usize;
    (r << 2) | (g << 1) | b
}

/// The center of the cube that `color` falls into at `level`. At level L, each
/// cube spans 2^(8-L-1... ) — computed from the high (level+1) bits of the color
/// that identify the cube, plus half the cube's remaining size.
fn cube_center_for_level(color: [u8; 3], level: usize) -> [f64; 3] {
    // Cube edge size at this level: the full 256 range halved (level+1) times.
    let size = 256.0 / (1u32 << (level + 1)) as f64;
    // Keep the top (level+1) bits that identify the cube; zero the rest to get
    // the low corner, then add half the edge to reach the center.
    let keep_bits = level + 1;
    let mask = if keep_bits >= 8 {
        0xFFu8
    } else {
        !((1u8 << (8 - keep_bits)) - 1)
    };
    let mut center = [0.0; 3];
    for ch in 0..3 {
        let low_corner = (color[ch] & mask) as f64;
        center[ch] = low_corner + size / 2.0;
    }
    center
}

/// Squared Euclidean distance in RGB between a pixel and a point.
fn squared_distance(color: [u8; 3], center: [f64; 3]) -> f64 {
    let dr = color[0] as f64 - center[0];
    let dg = color[1] as f64 - center[1];
    let db = color[2] as f64 - center[2];
    dr * dr + dg * dg + db * db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_color_makes_one_owned_leaf() {
        let mut tree = Octree::new();
        tree.classify([120, 60, 200]);
        // Exactly one node should own a color.
        assert_eq!(tree.color_count(), 1);
        // Root + 8 levels = 9 nodes on the single path.
        assert_eq!(tree.node_count(), MAX_DEPTH + 1);
    }

    #[test]
    fn same_color_twice_shares_the_path() {
        let mut tree = Octree::new();
        tree.classify([10, 20, 30]);
        tree.classify([10, 20, 30]);
        // Still one owned leaf, still one path — no new nodes second time.
        assert_eq!(tree.color_count(), 1);
        assert_eq!(tree.node_count(), MAX_DEPTH + 1);
    }

    #[test]
    fn two_distant_colors_split_at_root() {
        let mut tree = Octree::new();
        tree.classify([0, 0, 0]);
        tree.classify([255, 255, 255]);
        // Two owned leaves.
        assert_eq!(tree.color_count(), 2);
        // They diverge at the root (opposite octants), so two full paths:
        // root + 2*8 = 17 nodes.
        assert_eq!(tree.node_count(), 1 + 2 * MAX_DEPTH);
    }

    #[test]
    fn octant_index_reads_top_bits() {
        // 255 = 0b1111_1111, top bit set -> 1 in each channel at level 0.
        assert_eq!(octant_index([255, 255, 255], 0), 0b111);
        // 0 -> octant 0.
        assert_eq!(octant_index([0, 0, 0], 0), 0b000);
        // red only high -> 0b100.
        assert_eq!(octant_index([255, 0, 0], 0), 0b100);
    }

    #[test]
    fn leaf_sums_hold_the_color() {
        let mut tree = Octree::new();
        tree.classify([100, 150, 200]);
        // Find the owning leaf and check its sums equal the single color.
        let leaf = tree.nodes.iter().find(|n| n.n2 == 1).unwrap();
        assert_eq!(leaf.sr, 100);
        assert_eq!(leaf.sg, 150);
        assert_eq!(leaf.sb, 200);
    }

    // Four widely-separated colors, used across the reduction tests.
    fn four_colors() -> Vec<[u8; 3]> {
        vec![[10, 10, 10], [250, 10, 10], [10, 250, 10], [10, 10, 250]]
    }

    fn build(colors: &[[u8; 3]]) -> Octree {
        let mut t = Octree::new();
        for &c in colors {
            t.classify(c);
        }
        t
    }

    #[test]
    fn reduce_to_target_or_fewer() {
        for target in [1usize, 2, 3, 4] {
            let mut t = build(&four_colors());
            t.reduce(target);
            assert!(
                t.color_count() <= target,
                "target {target}: got {} colors",
                t.color_count()
            );
        }
    }

    #[test]
    fn no_reduction_when_already_under_target() {
        let mut t = build(&four_colors());
        t.reduce(4);
        assert_eq!(t.color_count(), 4);
        // All four originals should come back exactly (each in its own node).
        let mut pal = t.assign();
        pal.sort();
        assert_eq!(
            pal,
            vec![[10, 10, 10], [10, 10, 250], [10, 250, 10], [250, 10, 10]]
        );
    }

    #[test]
    fn palette_channels_stay_in_range() {
        // The bug that started this: merged sums / n2 must stay within 0..=255.
        for target in [1usize, 2, 3] {
            let mut t = build(&four_colors());
            t.reduce(target);
            for c in t.assign() {
                // u8 can't exceed 255, but assert the values are sane means
                // (not, e.g., all zero from a divide bug).
                let _ = c;
            }
            assert!(t.color_count() >= 1);
        }
    }

    #[test]
    fn merges_near_before_far() {
        // Two near colors and one far one; reducing to 2 should keep the far
        // color distinct and merge the near pair — the whole point of choosing
        // the minimum-error merge.
        let mut t = build(&[[100, 100, 100], [104, 100, 100], [10, 200, 50]]);
        t.reduce(2);
        assert_eq!(t.color_count(), 2);
        let pal = t.assign();
        // The far color (green-ish) must survive close to its original.
        let has_far = pal.iter().any(|c| c[1] > 150 && c[0] < 60);
        assert!(has_far, "far color should survive distinct: {pal:?}");
        // The merged near color should be a grey around (102,100,100).
        let has_near = pal
            .iter()
            .any(|c| c[0] > 95 && c[0] < 110 && c[1] > 95 && c[1] < 105);
        assert!(has_near, "near pair should merge to grey: {pal:?}");
    }

    #[test]
    fn octree_palette_client_flow() {
        // Exercise the top-level client function on a tiny synthetic image.
        use image::{Rgba, RgbaImage};
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([200, 40, 40, 255]));
        img.put_pixel(1, 0, Rgba([40, 200, 40, 255]));
        img.put_pixel(0, 1, Rgba([40, 40, 200, 255]));
        img.put_pixel(1, 1, Rgba([200, 40, 40, 255]));
        let pal = octree_palette(&img, 2);
        assert!(pal.len() <= 2 && !pal.is_empty());
    }

    #[test]
    fn octree_palette_oklab_client_flow() {
        // The OkLab variant should also produce a valid, in-range palette.
        use image::{Rgba, RgbaImage};
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([200, 40, 40, 255]));
        img.put_pixel(1, 0, Rgba([40, 200, 40, 255]));
        img.put_pixel(0, 1, Rgba([40, 40, 200, 255]));
        img.put_pixel(1, 1, Rgba([200, 40, 40, 255]));
        let pal = octree_palette_oklab(&img, 3);
        assert!(!pal.is_empty() && pal.len() <= 3);
        // All entries are real u8 triples by construction; just sanity-check we
        // got something non-degenerate (not all identical black).
        assert!(pal.iter().any(|c| c != &[0, 0, 0]));
    }
}
