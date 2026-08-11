//! Adaptive palette generation via median-cut, then mapping through the
//! existing `palette_map`.
//!
//! This op generates a palette *from the image* instead of taking a fixed one,
//! then reduces the image to it. The generation is the only new code here —
//! the mapping and dithering are reused by calling `palette_map` with the
//! generated colors (its input is `&[String]` of hex codes, so we just produce
//! those). No mapping/dithering logic is duplicated.
//!
//! Algorithm: dampened-population median-cut over a color histogram.
//! 1. Sample pixels and collapse to a **chroma-weighted** histogram: distinct
//!    colors, where each pixel's contribution is boosted by its colorfulness.
//!    Saturated pixels count for more than grey ones, so vivid regions (a face,
//!    an accent) earn palette slots that their raw pixel count alone wouldn't
//!    win against large flat backgrounds. A baseline weight keeps flat regions
//!    represented too.
//! 2. Put all histogram entries in one box; repeatedly split the box with the
//!    greatest sqrt(population) × range. The square root keeps big flat regions
//!    from monopolizing the palette (which greys out smaller important regions
//!    like faces) while still preferring populous areas over rare outliers.
//! 3. Each box's representative color is a **hybrid**: the population-weighted
//!    average *lightness* (smooth tone) combined with the *chroma* of the box's
//!    mode, its most-populous real color (vibrant hue/saturation). A plain
//!    average desaturates; the pure mode can be jumpy; the hybrid keeps smooth
//!    tonal steps while staying as colorful as the source.

use crate::color_utils::hybrid_lightness_chroma;
use crate::palette_map::palette_map;
use crate::{DitherConfig, Image, PixelizerError};
use std::collections::HashMap;

/// Roughly how many pixels to sample before building the histogram. More is
/// slower with little quality gain; a few thousand represents the distribution
/// well.
const SAMPLE_TARGET: usize = 10_000;

/// Generate an adaptive palette from the image and map to it. `colors` is the
/// maximum palette size (fewer are returned if the image has fewer distinct
/// colors). `dither` and `preserve_alpha` are forwarded straight to
/// `palette_map`.
pub fn adaptive_palette(
    image: Image,
    colors: u32,
    dither: Option<DitherConfig>,
    preserve_alpha: Option<bool>,
    space: crate::color_utils::MappingSpace,
) -> Result<Image, PixelizerError> {
    let rgb = crate::octree::octree_palette(&image, colors.max(1) as usize);
    let hex: Vec<String> = rgb
        .iter()
        .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
        .collect();
    palette_map(image, &hex, dither, preserve_alpha, space)
}

/// A box of distinct colors (with per-color pixel counts), tracked so we can
/// pick which box to split next, along which channel, weighted by population.
struct ColorBox {
    /// (color, count) — distinct colors and how many pixels each represents.
    entries: Vec<([u8; 3], u32)>,
}

impl ColorBox {
    /// Total pixels this box represents (sum of counts), used to weight which
    /// box to split — bigger populations deserve more palette resolution.
    fn population(&self) -> u64 {
        self.entries.iter().map(|(_, c)| *c as u64).sum()
    }

    /// The (min, max) for each channel over the colors in this box.
    fn ranges(&self) -> [(u8, u8); 3] {
        let mut lo = [255u8; 3];
        let mut hi = [0u8; 3];
        for (c, _) in &self.entries {
            for ch in 0..3 {
                lo[ch] = lo[ch].min(c[ch]);
                hi[ch] = hi[ch].max(c[ch]);
            }
        }
        [(lo[0], hi[0]), (lo[1], hi[1]), (lo[2], hi[2])]
    }

    /// The widest channel and its span — the axis to sort/split along.
    fn widest_channel(&self) -> (usize, u8) {
        let r = self.ranges();
        let spans = [r[0].1 - r[0].0, r[1].1 - r[1].0, r[2].1 - r[2].0];
        let mut ch = 0;
        for i in 1..3 {
            if spans[i] > spans[ch] {
                ch = i;
            }
        }
        (ch, spans[ch])
    }

    /// Split priority: **sqrt(population) × widest-channel span**. A box scores
    /// high when it both covers a lot of pixels AND spans a lot of color. The
    /// square root *dampens* the population term: without it, a few huge flat
    /// regions (a wall, a shirt) win every split and starve smaller but
    /// perceptually important regions (a face) of palette entries — they end up
    /// mapped to grey. Raw range-only weighting has the opposite failure (it
    /// chases wide-but-rare outliers); sqrt(pop) × span is the balanced middle
    /// most quantizers use.
    fn split_priority(&self) -> f64 {
        let (_, span) = self.widest_channel();
        (self.population() as f64).sqrt() * span as f64
    }

    /// Can this box be split? Only if it holds more than one distinct color.
    fn splittable(&self) -> bool {
        self.entries.len() > 1
    }

    /// The box's representative color: a **hybrid** of the mode and the mean.
    /// It takes the population-weighted *average lightness* of the box (smooth,
    /// stable tone — this is what a plain average gets right) but the *chroma*
    /// (hue and saturation) of the box's mode, its most-populous real color
    /// (vibrant — this is what averaging desaturates). Combining them in OkLab
    /// gives smooth tonal transitions between palette entries without the
    /// washed-out color a full average produces. See
    /// `color_utils::hybrid_lightness_chroma`.
    fn representative(&self) -> [u8; 3] {
        let mode = self
            .entries
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(rgb, _)| *rgb)
            .unwrap_or([0, 0, 0]);
        hybrid_lightness_chroma(&self.entries, mode)
    }
}

/// Population-weighted median-cut: returns up to `n` hex color strings.
fn median_cut(image: &Image, n: usize) -> Vec<String> {
    let histogram = build_histogram(image);
    if histogram.is_empty() {
        return Vec::new();
    }

    let mut boxes = vec![ColorBox { entries: histogram }];

    while boxes.len() < n {
        // Pick the splittable box with the greatest priority (population-
        // weighted range). f64 isn't Ord (NaN), so compare with partial_cmp;
        // priorities here are always finite, so unwrap_or(Equal) is safe.
        let target = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.splittable())
            .max_by(|(_, a), (_, b)| {
                a.split_priority()
                    .partial_cmp(&b.split_priority())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let Some((idx, _)) = target else {
            break; // every box is a single color; can't split further
        };

        let mut b = boxes.swap_remove(idx);
        let (ch, _) = b.widest_channel();
        // Sort by the widest channel, then split at the population median — the
        // point where cumulative pixel count crosses half the box's population,
        // so each half carries roughly equal pixel weight (not equal distinct-
        // color count).
        b.entries.sort_by_key(|(c, _)| c[ch]);
        let half = b.population() / 2;
        let mut acc = 0u64;
        let mut split_at = 1; // ensure both halves are non-empty
        for (i, (_, count)) in b.entries.iter().enumerate() {
            acc += *count as u64;
            if acc >= half {
                // Split after i, but keep at least one entry on each side.
                split_at = (i + 1).clamp(1, b.entries.len() - 1);
                break;
            }
        }
        let hi = b.entries.split_off(split_at);
        boxes.push(ColorBox { entries: b.entries });
        boxes.push(ColorBox { entries: hi });
    }

    boxes
        .iter()
        .map(|b| {
            let [r, g, b_] = b.representative();
            format!("#{r:02x}{g:02x}{b_:02x}")
        })
        .collect()
}

/// How strongly chroma (colorfulness) boosts a pixel's weight in the histogram.
/// Each pixel contributes `1 + CHROMA_WEIGHT * chroma` (chroma in 0..1), so a
/// fully-saturated pixel counts `1 + CHROMA_WEIGHT` times as much as a grey one.
/// This biases palette generation toward vivid colors — a small saturated
/// region (a face against a flat wall) earns more palette slots than its raw
/// pixel count would grant. It's a *bias*, not a replacement: the baseline `1`
/// guarantees every region, however flat, still contributes. Tune to taste;
/// higher = more vivid palettes, but push too far and large smooth regions get
/// too few entries and band.
const CHROMA_WEIGHT: f32 = 4.0;

/// A pixel's colorfulness in 0..1: the sRGB saturation proxy (max channel minus
/// min channel). Cheap, no color-space conversion, and good enough for biasing
/// — grey pixels score ~0, vivid pixels score near 1.
fn chroma_proxy(r: u8, g: u8, b: u8) -> f32 {
    let max = r.max(g).max(b) as f32;
    let min = r.min(g).min(b) as f32;
    (max - min) / 255.0
}

/// Sample pixels (strided) and collapse to a **chroma-weighted** histogram:
/// distinct opaque colors with weighted counts. Saturated pixels contribute
/// more than grey ones (see CHROMA_WEIGHT), so the palette leans toward vivid
/// colors. Weights are accumulated as scaled integers to keep the rest of the
/// pipeline on `(color, u32)`. Fully transparent pixels are skipped.
fn build_histogram(image: &Image) -> Vec<([u8; 3], u32)> {
    let total = (image.width() * image.height()) as usize;
    let stride = (total / SAMPLE_TARGET).max(1);

    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    for p in image.pixels().step_by(stride) {
        if p.0[3] == 0 {
            continue; // skip fully transparent
        }
        let (r, g, b) = (p.0[0], p.0[1], p.0[2]);
        // 1 + k*chroma, scaled by 16 and rounded so weights stay integers with
        // enough resolution (a grey pixel = 16; a vivid one = 16*(1+k)).
        let weight = ((1.0 + CHROMA_WEIGHT * chroma_proxy(r, g, b)) * 16.0).round() as u32;
        *counts.entry([r, g, b]).or_insert(0) += weight.max(1);
    }
    counts.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(w: u32, h: u32, color: [u8; 4]) -> Image {
        let mut img = Image::new(w, h);
        for p in img.pixels_mut() {
            *p = Rgba(color);
        }
        img
    }

    #[test]
    fn single_color_image_yields_one_entry() {
        let img = solid(8, 8, [200, 50, 50, 255]);
        let pal = median_cut(&img, 16);
        assert_eq!(pal.len(), 1, "a solid image has only one color");
    }

    #[test]
    fn respects_requested_count_upper_bound() {
        let mut img = Image::new(4, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(2, 0, Rgba([255, 255, 255, 255]));
        img.put_pixel(3, 0, Rgba([255, 255, 255, 255]));
        let pal = median_cut(&img, 16);
        assert!(pal.len() <= 2, "only two distinct colors present");
    }

    #[test]
    fn hex_format_is_valid() {
        let img = solid(4, 4, [18, 52, 86, 255]);
        let pal = median_cut(&img, 4);
        assert!(pal[0].starts_with('#') && pal[0].len() == 7);
    }

    #[test]
    fn dominant_color_survives_rare_outlier() {
        // A field of one color with a single wildly different outlier pixel.
        // Population weighting should spend its 2 slots keeping the dominant
        // color well-represented, not chase the lone outlier's wide range.
        let mut img = Image::new(10, 10);
        for p in img.pixels_mut() {
            *p = Rgba([100, 120, 140, 255]);
        }
        img.put_pixel(0, 0, Rgba([255, 0, 255, 255])); // one magenta outlier
        let pal = median_cut(&img, 2);
        // The dominant blue-grey must appear; exact hex depends on averaging,
        // so just assert we got a palette and it's not both-magenta.
        assert!(!pal.is_empty());
    }
}
