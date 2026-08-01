pub use image;
mod blur;
mod color_utils;
mod downsample;
mod normalize;
pub mod op_schema;
mod palette_map;
mod posterize;
mod upscale;
use blur::blur;
use downsample::downsample;
use normalize::normalize;
use palette_map::palette_map;
use posterize::posterize;
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
}

fn default_low_percentile() -> f32 {
    0.01
}
fn default_high_percentile() -> f32 {
    0.99
} // clip brightest 1%

/// Apply one operation. The single dispatch point, shared by `apply` and
/// `apply_stages` so the match can't drift between them.
fn apply_one(op: &Operation, image: Image) -> Result<Image, PixelizerError> {
    Ok(match op {
        Operation::Downsample { pixel_size } => downsample(*pixel_size, image),
        Operation::PaletteMap {
            colors,
            dither,
            preserve_alpha,
        } => palette_map(image, colors, *dither, *preserve_alpha)?,
        Operation::Upscale { factor } => upscale(image, *factor),
        Operation::Posterize { levels } => posterize(image, *levels)?,
        Operation::Blur { sigma } => blur(image, *sigma),
        Operation::Normalize { low, high } => normalize(image, *low, *high),
    })
}

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
        stages.push(image.clone()); // keep this stage; `image` continues to the next op
    }
    Ok(stages)
}
