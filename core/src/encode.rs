//! PNG output encoding, size-optimized for pixel art.
//!
//! Pixelizer's outputs usually have few distinct colors (that's the point). A
//! truecolor PNG stores 4 bytes (RGBA) per pixel regardless — you pay for a
//! 16-million-color space you aren't using. PNG's *indexed* mode instead stores
//! the palette once (the PLTE chunk) and each pixel as a small *index* into it.
//! For a 16-color image that's 4 bits per pixel instead of 32 — an 8× cut
//! before compression, and indexed data compresses better too.
//!
//! The catch: an index is at most 8 bits, so indexed mode caps at 256 palette
//! entries. And this first version handles only *opaque* images — an indexed
//! PNG with no `tRNS` chunk is fully opaque by definition, so skipping alpha
//! lets us skip a whole chunk of code. Any transparency, or more than 256
//! colors, falls back to plain truecolor (correct, just not smaller).
//!
//! `image`'s `write_to(_, Png)` always writes truecolor, so the indexed path
//! uses the lower-level `png` crate directly.

use crate::Image;
use std::collections::HashMap;
use std::io::Cursor;

/// Encode an image to PNG bytes, choosing indexed color when it's both small
/// enough (≤256 distinct colors) and fully opaque; otherwise truecolor.
pub fn encode_png(image: &Image) -> Vec<u8> {
    match indexed_plan(image) {
        Some(plan) => encode_indexed(image, &plan),
        None => encode_truecolor(image),
    }
}

/// The data an indexed encode needs: the palette (distinct RGB colors, in a
/// fixed order) and a lookup from color to its palette index.
struct IndexedPlan {
    palette: Vec<[u8; 3]>,
    index_of: HashMap<[u8; 3], u8>,
}

/// Decide whether the image qualifies for indexed encoding, and if so, build
/// its palette. Returns None if any pixel is non-opaque or if there are more
/// than 256 distinct colors — either disqualifies indexed mode.
fn indexed_plan(image: &Image) -> Option<IndexedPlan> {
    let mut index_of: HashMap<[u8; 3], u8> = HashMap::new();
    let mut palette: Vec<[u8; 3]> = Vec::new();

    for px in image.pixels() {
        let [r, g, b, a] = px.0;
        // Any transparency disqualifies this simple opaque-only path.
        if a != 255 {
            return None;
        }
        let rgb = [r, g, b];
        // First time we see a color, it gets the next palette slot. Once we'd
        // need a 257th entry, indexed mode can't represent it — bail.
        if !index_of.contains_key(&rgb) {
            if palette.len() == 256 {
                return None;
            }
            index_of.insert(rgb, palette.len() as u8);
            palette.push(rgb);
        }
    }

    Some(IndexedPlan { palette, index_of })
}

/// Write an indexed PNG: a PLTE chunk (the palette) plus one index byte per
/// pixel. No tRNS chunk, so the image is opaque.
fn encode_indexed(image: &Image, plan: &IndexedPlan) -> Vec<u8> {
    let (w, h) = image.dimensions();

    // PLTE wants a flat [r,g,b, r,g,b, ...] byte run.
    let mut plte = Vec::with_capacity(plan.palette.len() * 3);
    for c in &plan.palette {
        plte.extend_from_slice(c);
    }

    // One index byte per pixel, row-major — same order as PLTE expects.
    let mut indices = Vec::with_capacity((w * h) as usize);
    for px in image.pixels() {
        let rgb = [px.0[0], px.0[1], px.0[2]];
        // Every color was inserted during planning, so this can't miss.
        indices.push(plan.index_of[&rgb]);
    }

    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut out), w, h);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight); // 8-bit indices (up to 256 entries)
        encoder.set_palette(plte);
        let mut writer = encoder.write_header().expect("PNG header");
        writer.write_image_data(&indices).expect("PNG indexed data");
    }
    out
}

/// Fallback: plain 8-bit RGBA truecolor. Used when the image has transparency
/// or too many colors for indexed mode.
fn encode_truecolor(image: &Image) -> Vec<u8> {
    let (w, h) = image.dimensions();
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut out), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header");
        writer
            .write_image_data(image.as_raw())
            .expect("PNG rgba data");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn few_colors_opaque_uses_indexed() {
        // 2×2, two opaque colors → qualifies.
        let mut img = Image::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 255, 255]));
        img.put_pixel(0, 1, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 1, Rgba([0, 0, 255, 255]));

        let plan = indexed_plan(&img).expect("should qualify");
        assert_eq!(plan.palette.len(), 2);

        // The indexed encoding should be smaller than truecolor for this.
        let indexed = encode_png(&img);
        let truecolor = encode_truecolor(&img);
        assert!(
            indexed.len() <= truecolor.len(),
            "indexed {} should not exceed truecolor {}",
            indexed.len(),
            truecolor.len()
        );
    }

    #[test]
    fn transparency_forces_truecolor() {
        let mut img = Image::new(1, 1);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 128])); // semi-transparent
        assert!(indexed_plan(&img).is_none());
    }

    #[test]
    fn indexed_png_decodes_back_to_same_pixels() {
        // Round-trip: encode indexed, decode, compare. Guards the palette/index
        // wiring — a wrong index would surface as swapped colors.
        let mut img = Image::new(3, 1);
        img.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
        img.put_pixel(1, 0, Rgba([40, 50, 60, 255]));
        img.put_pixel(2, 0, Rgba([10, 20, 30, 255]));

        let bytes = encode_png(&img);
        let decoded = image::load_from_memory(&bytes).expect("decode").to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0, [10, 20, 30, 255]);
        assert_eq!(decoded.get_pixel(1, 0).0, [40, 50, 60, 255]);
        assert_eq!(decoded.get_pixel(2, 0).0, [10, 20, 30, 255]);
    }
}
