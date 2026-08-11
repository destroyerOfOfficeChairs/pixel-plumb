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

/// Tree depth. 8 levels below the root lets every distinct 8-bit RGB color reach
/// its own leaf (one bit per level per channel). The article uses log2(Cmax+1).
const MAX_DEPTH: usize = 8;

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
        }
    }
}

/// The octree: an arena of nodes plus the root index (always 0).
pub struct Octree {
    nodes: Vec<Node>,
    root: usize,
}

impl Octree {
    /// A fresh tree with just the root node.
    pub fn new() -> Self {
        Octree {
            nodes: vec![Node::new(None)],
            root: 0,
        }
    }

    /// Classify one pixel: walk root→leaf, accumulating stats on every node
    /// along the path, creating child nodes lazily. The deepest node gets n2.
    pub fn classify(&mut self, color: [u8; 3]) {
        let mut idx = self.root;

        for level in 0..MAX_DEPTH {
            // Accumulate this pixel into the current node: counts, color sums,
            // and squared error to this node's cube center.
            {
                let node = &mut self.nodes[idx];
                node.n1 += 1;
                node.sr += color[0] as u64;
                node.sg += color[1] as u64;
                node.sb += color[2] as u64;
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

        // idx is now the leaf for this exact color: it owns the pixel.
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

    /// Count nodes that currently own a color (n2 > 0). This is the number of
    /// output colors; reduction drives it down to the target.
    pub fn color_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.n2 > 0).count()
    }

    /// Total node count (for tests/inspection).
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    // fn reduce(&mut self, target: usize) { ... }
    // fn assign(&self) -> Vec<[u8; 3]> { ... }         // &self: read-only
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
}
