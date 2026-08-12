pub use image;
mod adaptive_palette;
mod blur;
mod color_utils;
mod contrast;
mod downsample;
mod encode;
mod normalize;
mod octree;
pub mod op_schema;
mod palette_map;
mod pixelizer_resize;
mod posterize;
mod saturation;
mod upscale;
use adaptive_palette::adaptive_palette;
use blur::blur;
pub use color_utils::MappingSpace;
use contrast::contrast;
use downsample::downsample;
pub use encode::encode_png;
use normalize::normalize;
use palette_map::palette_map;
use pixelizer_resize::pixelizer_resize;
use posterize::posterize;
use saturation::saturation;
use upscale::upscale;

pub type Image = image::RgbaImage;

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy)]
#[serde(tag = "algorithm", rename_all = "snake_case")]
pub enum DitherConfig {
    FloydSteinberg {
        #[serde(default = "default_bleed")]
        bleed: f32,
        #[serde(default = "default_clamp")]
        clamp: bool,
    },
    Atkinson {
        #[serde(default = "default_bleed")]
        bleed: f32,
        #[serde(default = "default_clamp")]
        clamp: bool,
    },
    #[serde(rename = "jjn")]
    Jjn {
        #[serde(default = "default_bleed")]
        bleed: f32,
        #[serde(default = "default_clamp")]
        clamp: bool,
    },
    Bayer4 {
        #[serde(default = "default_strength")]
        strength: f32,
    },
    Bayer8 {
        #[serde(default = "default_strength")]
        strength: f32,
    },
}

fn default_clamp() -> bool {
    true
}

fn default_bleed() -> f32 {
    1.0
}
fn default_strength() -> f32 {
    32.0
}

#[derive(Debug)]
pub enum PixelizerError {
    HexParseError(String),
    NoColorsError(String),
    PosterizeError(String),
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Pipeline {
    pub operations: Vec<Operation>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Operation {
    Downsample {
        pixel_size: u32,
    },
    PaletteMap {
        colors: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dither: Option<DitherConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preserve_alpha: Option<bool>,
        #[serde(default)]
        mapping_space: MappingSpace,
    },
    AdaptivePaletteMap {
        #[serde(default = "default_adaptive_colors")]
        colors: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dither: Option<DitherConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preserve_alpha: Option<bool>,
        #[serde(default)]
        mapping_space: MappingSpace,
    },
    Upscale {
        factor: u32,
    },
    Posterize {
        levels: u32,
    },
    Blur {
        sigma: f32,
    },
    Normalize {
        #[serde(default = "default_low_percentile")]
        low: f32,
        #[serde(default = "default_high_percentile")]
        high: f32,
    },
    Saturation {
        #[serde(default = "default_saturation")]
        factor: f32,
    },
    Contrast {
        #[serde(default = "default_contrast")]
        factor: f32,
    },
    #[serde(rename = "resize")]
    PixelizerResize {
        #[serde(default)]
        exact: bool,
        #[serde(default = "default_resize_dim")]
        max_size: u32,
        #[serde(default = "default_resize_dim")]
        width: u32,
        #[serde(default = "default_resize_dim")]
        height: u32,
    },
}

fn default_resize_dim() -> u32 {
    64
}

fn default_adaptive_colors() -> u32 {
    16
}

fn default_saturation() -> f32 {
    1.0
}

fn default_contrast() -> f32 {
    1.0
}

fn default_low_percentile() -> f32 {
    0.01
}
fn default_high_percentile() -> f32 {
    0.99
} // clip brightest 1%

/// Apply one operation. The single dispatch point, shared by `apply` and
/// `apply_stages` so the match can't drift between them. The palette-map ops
/// carry their own `mapping_space`; every other op ignores color-space choice.
fn apply_one(op: &Operation, image: Image) -> Result<Image, PixelizerError> {
    Ok(match op {
        Operation::Downsample { pixel_size } => downsample(*pixel_size, image),
        Operation::PaletteMap {
            colors,
            dither,
            preserve_alpha,
            mapping_space,
        } => palette_map(image, colors, *dither, *preserve_alpha, *mapping_space)?,
        Operation::AdaptivePaletteMap {
            colors,
            dither,
            preserve_alpha,
            mapping_space,
        } => adaptive_palette(image, *colors, *dither, *preserve_alpha, *mapping_space)?,
        Operation::Upscale { factor } => upscale(image, *factor),
        Operation::Posterize { levels } => posterize(image, *levels)?,
        Operation::Blur { sigma } => blur(image, *sigma),
        Operation::Normalize { low, high } => normalize(image, *low, *high),
        Operation::Saturation { factor } => saturation(image, *factor),
        Operation::Contrast { factor } => contrast(image, *factor),
        Operation::PixelizerResize {
            exact,
            max_size,
            width,
            height,
        } => pixelizer_resize(image, *exact, *max_size, *width, *height),
    })
}

/// Run the whole pipeline. Palette-mapping color space is per-op (carried in the
/// `Operation`), so there's no pipeline-wide space argument anymore.
pub fn apply(pipeline: &Pipeline, mut image: Image) -> Result<Image, PixelizerError> {
    for op in &pipeline.operations {
        image = apply_one(op, image)?;
    }
    Ok(image)
}

/// Run the pipeline, returning the image after *each* operation, in order.
/// `stages[i]` is the result of ops `0..=i`. One clone per stage.
pub fn apply_stages(pipeline: &Pipeline, mut image: Image) -> Result<Vec<Image>, PixelizerError> {
    let mut stages = Vec::with_capacity(pipeline.operations.len());
    for op in &pipeline.operations {
        image = apply_one(op, image)?;
        stages.push(image.clone());
    }
    Ok(stages)
}
