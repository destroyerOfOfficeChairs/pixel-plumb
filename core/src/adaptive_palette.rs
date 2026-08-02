//! Adaptive palette generation via median-cut, then mapping through the
//! existing `palette_map`.
//!
//! This op generates a palette *from the image* instead of taking a fixed one,
//! then reduces the image to it. The generation is the only new code here —
//! the mapping and dithering are reused by calling `palette_map` with the
//! generated colors (its input is `&[String]` of hex codes, so we just produce
//! those). No mapping/dithering logic is duplicated.
//!
//! Algorithm: median-cut. Subsample the pixels (a few thousand capture the
//! color distribution fine and keep it fast), place them in one box, then
//! repeatedly split the box with the widest color range — along its widest
//! channel, at that channel's median — until we have the requested number of
//! boxes (or no box can be split further). Each box's average colour is a
//! palette entry.

use crate::color_utils::srgb_to_linear;
use crate::palette_map::palette_map;
use crate::{DitherConfig, Image, PixelizerError};

/// Roughly how many pixels to sample for palette generation. More is slower
/// with little quality gain; a few thousand represents the distribution well.
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
) -> Result<Image, PixelizerError> {
    let palette = median_cut(&image, colors.max(1) as usize);
    palette_map(image, &palette, dither, preserve_alpha)
}

/// A box of colors in RGB space, with its bounding range tracked so we can pick
/// which box to split next and along which channel.
struct ColorBox {
    colors: Vec<[u8; 3]>,
}

impl ColorBox {
    /// The (min, max) for each channel over the colors in this box.
    fn ranges(&self) -> [(u8, u8); 3] {
        let mut lo = [255u8; 3];
        let mut hi = [0u8; 3];
        for c in &self.colors {
            for ch in 0..3 {
                lo[ch] = lo[ch].min(c[ch]);
                hi[ch] = hi[ch].max(c[ch]);
            }
        }
        [(lo[0], hi[0]), (lo[1], hi[1]), (lo[2], hi[2])]
    }

    /// The widest channel and its span — used to rank boxes (widest span splits
    /// first) and to choose the axis to sort/split along.
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

    /// Average color of the box, in linear light. Averaging in linear space
    /// (not raw sRGB) gives a perceptually truer mean — consistent with the
    /// rest of pixelizer's colour handling.
    fn average(&self) -> [u8; 3] {
        let n = self.colors.len().max(1) as f64;
        let mut acc = [0f64; 3];
        for c in &self.colors {
            for ch in 0..3 {
                acc[ch] += srgb_to_linear(c[ch]) as f64;
            }
        }
        let mut out = [0u8; 3];
        for ch in 0..3 {
            let lin = (acc[ch] / n) as f32;
            out[ch] = linear_to_srgb_u8(lin);
        }
        out
    }
}

/// Inverse of `srgb_to_linear` for a single channel, to 8-bit. (color_utils'
/// linear_to_srgb is private; this local copy keeps the module self-contained.)
fn linear_to_srgb_u8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Median-cut: returns up to `n` hex color strings sampled from the image.
fn median_cut(image: &Image, n: usize) -> Vec<String> {
    let samples = sample_pixels(image);
    if samples.is_empty() {
        return Vec::new();
    }

    let mut boxes = vec![ColorBox { colors: samples }];

    // Split until we have n boxes or nothing can be split further.
    while boxes.len() < n {
        // Pick the box with the widest channel span (and >1 color to split).
        let target = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.colors.len() > 1)
            .max_by_key(|(_, b)| b.widest_channel().1);

        let Some((idx, _)) = target else {
            break; // every box is a single color; can't split further
        };

        let mut b = boxes.swap_remove(idx);
        let (ch, _) = b.widest_channel();
        // Sort along the widest channel and split at the median.
        b.colors.sort_by_key(|c| c[ch]);
        let mid = b.colors.len() / 2;
        let hi = b.colors.split_off(mid);
        boxes.push(ColorBox { colors: b.colors });
        boxes.push(ColorBox { colors: hi });
    }

    boxes
        .iter()
        .map(|b| {
            let [r, g, b_] = b.average();
            format!("#{r:02x}{g:02x}{b_:02x}")
        })
        .collect()
}

/// Take up to ~SAMPLE_TARGET opaque pixels, evenly strided across the image.
/// Fully transparent pixels are skipped (they'd pollute the palette with
/// whatever RGB sits under alpha 0).
fn sample_pixels(image: &Image) -> Vec<[u8; 3]> {
    let pixels = image.pixels();
    let total = (image.width() * image.height()) as usize;
    let stride = (total / SAMPLE_TARGET).max(1);

    pixels
        .step_by(stride)
        .filter(|p| p.0[3] > 0)
        .map(|p| [p.0[0], p.0[1], p.0[2]])
        .collect()
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
        // A two-tone image can't yield more than 2 entries even if asked for 16.
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
}
