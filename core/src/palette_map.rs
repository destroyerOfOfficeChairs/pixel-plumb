use crate::DitherConfig;
use crate::Image;
use std::collections::HashMap;

use crate::color_utils::BayerMatrix;
use crate::color_utils::Oklab;
use crate::color_utils::PaletteData;
use crate::color_utils::nearest_oklab;
use crate::color_utils::nearest_rgb;
use crate::color_utils::prepare_palette;
use crate::color_utils::quantize;
use crate::color_utils::rgb_to_oklab;
use crate::color_utils::srgb_to_linear;

const FLOYD_STEINBERG: &[(i32, i32, f32)] = &[
    (1, 0, 7.0 / 16.0),
    (-1, 1, 3.0 / 16.0),
    (0, 1, 5.0 / 16.0),
    (1, 1, 1.0 / 16.0),
];

const ATKINSON: &[(i32, i32, f32)] = &[
    (1, 0, 1.0 / 8.0),
    (2, 0, 1.0 / 8.0),
    (-1, 1, 1.0 / 8.0),
    (0, 1, 1.0 / 8.0),
    (1, 1, 1.0 / 8.0),
    (0, 2, 1.0 / 8.0),
];

const JJN: &[(i32, i32, f32)] = &[
    (1, 0, 7.0 / 48.0),
    (2, 0, 5.0 / 48.0),
    (-2, 1, 3.0 / 48.0),
    (-1, 1, 5.0 / 48.0),
    (0, 1, 7.0 / 48.0),
    (1, 1, 5.0 / 48.0),
    (2, 1, 3.0 / 48.0),
    (-2, 2, 1.0 / 48.0),
    (-1, 2, 3.0 / 48.0),
    (0, 2, 5.0 / 48.0),
    (1, 2, 3.0 / 48.0),
    (2, 2, 1.0 / 48.0),
];

// 4x4 Bayer matrix, normalized to [-0.5, 0.5) range
const BAYER_4X4: [[f32; 4]; 4] = [
    [-0.5, 0.0, -0.375, 0.125],
    [0.25, -0.25, 0.375, -0.125],
    [-0.3125, 0.1875, -0.4375, 0.0625],
    [0.4375, -0.0625, 0.3125, -0.1875],
];

const BAYER_8X8: [[f32; 8]; 8] = [
    // Values are (n / 64) - 0.5 where n is from the standard Bayer-8 integer matrix
    // [ 0, 32,  8, 40,  2, 34, 10, 42],
    // [48, 16, 56, 24, 50, 18, 58, 26],
    // [12, 44,  4, 36, 14, 46,  6, 38],
    // [60, 28, 52, 20, 62, 30, 54, 22],
    // [ 3, 35, 11, 43,  1, 33,  9, 41],
    // [51, 19, 59, 27, 49, 17, 57, 25],
    // [15, 47,  7, 39, 13, 45,  5, 37],
    // [63, 31, 55, 23, 61, 29, 53, 21],
    // After (n/64 - 0.5):
    [
        -0.5, 0.0, -0.375, 0.125, -0.46875, 0.03125, -0.34375, 0.15625,
    ],
    [
        0.25, -0.25, 0.375, -0.125, 0.28125, -0.21875, 0.40625, -0.09375,
    ],
    [
        -0.3125, 0.1875, -0.4375, 0.0625, -0.28125, 0.21875, -0.40625, 0.09375,
    ],
    [
        0.4375, -0.0625, 0.3125, -0.1875, 0.46875, -0.03125, 0.34375, -0.15625,
    ],
    [
        -0.453125, 0.046875, -0.328125, 0.171875, -0.484375, 0.015625, -0.359375, 0.140625,
    ],
    [
        0.296875, -0.203125, 0.421875, -0.078125, 0.265625, -0.234375, 0.390625, -0.109375,
    ],
    [
        -0.265625, 0.234375, -0.390625, 0.109375, -0.296875, 0.203125, -0.421875, 0.078125,
    ],
    [
        0.484375, -0.015625, 0.359375, -0.140625, 0.453125, -0.046875, 0.328125, -0.171875,
    ],
];

pub fn palette_map(
    image: Image,
    colors: &[String],
    dither: Option<DitherConfig>,
    preserve_alpha: Option<bool>,
    space: crate::color_utils::MappingSpace,
) -> Result<Image, crate::PixelizerError> {
    let foo: PaletteData = prepare_palette(colors)?;
    match dither {
        None => palette_map_flat(image, foo.rgb, foo.lab, preserve_alpha, space),
        Some(DitherConfig::FloydSteinberg { bleed, clamp }) => palette_map_diffuse(
            image,
            FLOYD_STEINBERG,
            foo.rgb,
            foo.lab,
            foo.linear,
            foo.max_per_channel,
            bleed,
            clamp,
            preserve_alpha,
            space,
        ),
        Some(DitherConfig::Atkinson { bleed, clamp }) => palette_map_diffuse(
            image,
            ATKINSON,
            foo.rgb,
            foo.lab,
            foo.linear,
            foo.max_per_channel,
            bleed,
            clamp,
            preserve_alpha,
            space,
        ),
        Some(DitherConfig::Jjn { bleed, clamp }) => palette_map_diffuse(
            image,
            JJN,
            foo.rgb,
            foo.lab,
            foo.linear,
            foo.max_per_channel,
            bleed,
            clamp,
            preserve_alpha,
            space,
        ),
        Some(DitherConfig::Bayer4 { strength }) => {
            palette_map_ordered(image, foo.rgb, foo.lab, strength, 4, preserve_alpha, space)
        }
        Some(DitherConfig::Bayer8 { strength }) => {
            palette_map_ordered(image, foo.rgb, foo.lab, strength, 8, preserve_alpha, space)
        }
    }
}

pub fn palette_map_flat(
    image: Image,
    rgb: Vec<[u8; 3]>,
    lab: Vec<Oklab>,
    preserve_alpha: Option<bool>,
    space: crate::color_utils::MappingSpace,
) -> Result<Image, crate::PixelizerError> {
    let (w, h) = image.dimensions();
    let mut out = Image::new(w, h);

    let mut cache: HashMap<[u8; 3], usize> = HashMap::new();

    for (x, y, pixel) in image.enumerate_pixels() {
        let [r, g, b, mut a] = pixel.0;
        if !preserve_alpha.unwrap_or_default() {
            a = 255;
        }
        let idx = *cache.entry([r, g, b]).or_insert_with(|| match space {
            crate::color_utils::MappingSpace::Oklab => nearest_oklab(&lab, rgb_to_oklab(r, g, b)),
            crate::color_utils::MappingSpace::Rgb => nearest_rgb(&rgb, [r, g, b]),
        });
        let [pr, pg, pb] = rgb[idx];
        out.put_pixel(x, y, image::Rgba([pr, pg, pb, a]));
    }
    Ok(out)
}

pub fn palette_map_diffuse(
    img: Image,
    alg: &[(i32, i32, f32)],
    rgb: Vec<[u8; 3]>,
    lab: Vec<Oklab>,
    linear: Vec<[f32; 3]>,
    max_per_channel: [f32; 3],
    bleed: f32,
    clamp: bool,
    preserve_alpha: Option<bool>,
    space: crate::color_utils::MappingSpace,
) -> Result<Image, crate::PixelizerError> {
    let (w, h) = img.dimensions();

    // Working buffer in LINEAR light, not sRGB.
    let mut buf: Vec<[f32; 3]> = img
        .pixels()
        .map(|p| {
            [
                srgb_to_linear(p.0[0]),
                srgb_to_linear(p.0[1]),
                srgb_to_linear(p.0[2]),
            ]
        })
        .collect();

    let alpha: Vec<u8> = img
        .pixels()
        .map(|p| {
            if preserve_alpha.unwrap_or_default() {
                p.0[3]
            } else {
                255
            }
        })
        .collect();
    let mut out = Image::new(w, h);

    let idx = |x: u32, y: u32| (y * w + x) as usize;

    for y in 0..h {
        let ltr = y % 2 == 0;
        let sign = if ltr { 1 } else { -1 };
        let xs: Box<dyn Iterator<Item = u32>> = if ltr {
            Box::new(0..w)
        } else {
            Box::new((0..w).rev())
        };

        for x in xs {
            let pixel = if clamp {
                let p = buf[idx(x, y)];
                [
                    p[0].clamp(0.0, max_per_channel[0]),
                    p[1].clamp(0.0, max_per_channel[1]),
                    p[2].clamp(0.0, max_per_channel[2]),
                ]
            } else {
                buf[idx(x, y)]
            };
            let (pal_idx, error) = quantize(&lab, &rgb, &linear, pixel, bleed, space);
            let [pr, pg, pb] = rgb[pal_idx];
            out.put_pixel(x, y, image::Rgba([pr, pg, pb, alpha[idx(x, y)]]));

            for &(dx, dy, weight) in alg {
                let nx = x as i32 + dx * sign;
                let ny = y as i32 + dy;
                if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 {
                    continue;
                }
                let p = &mut buf[idx(nx as u32, ny as u32)];
                for c in 0..3 {
                    p[c] += error[c] * weight;
                }
            }
        }
    }
    Ok(out)
}

pub fn palette_map_ordered(
    image: Image,
    rgb: Vec<[u8; 3]>,
    lab: Vec<Oklab>,
    strength: f32,
    size: usize,
    preserve_alpha: Option<bool>,
    space: crate::color_utils::MappingSpace,
) -> Result<Image, crate::PixelizerError> {
    let (w, h) = image.dimensions();
    let mut out = Image::new(w, h);

    let matrix = if size == 4 {
        BayerMatrix::Four(&BAYER_4X4)
    } else {
        BayerMatrix::Eight(&BAYER_8X8)
    };

    for (x, y, pixel) in image.enumerate_pixels() {
        let [r, g, b, mut a] = pixel.0;
        if !preserve_alpha.unwrap_or_default() {
            a = 255
        };
        let bias = match matrix {
            BayerMatrix::Four(m) => m[(y as usize) % size][(x as usize) % size] * strength,
            BayerMatrix::Eight(m) => m[(y as usize) % size][(x as usize) % size] * strength,
        };

        let biased_r = (r as f32 + bias).clamp(0.0, 255.0) as u8;
        let biased_g = (g as f32 + bias).clamp(0.0, 255.0) as u8;
        let biased_b = (b as f32 + bias).clamp(0.0, 255.0) as u8;

        let idx = match space {
            crate::color_utils::MappingSpace::Oklab => {
                nearest_oklab(&lab, rgb_to_oklab(biased_r, biased_g, biased_b))
            }
            crate::color_utils::MappingSpace::Rgb => {
                nearest_rgb(&rgb, [biased_r, biased_g, biased_b])
            }
        };
        let [pr, pg, pb] = rgb[idx];
        out.put_pixel(x, y, image::Rgba([pr, pg, pb, a]));
    }
    Ok(out)
}
