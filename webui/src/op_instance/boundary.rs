//! The boundary conversion — the ONE place the typed enum is reconstructed,
//! and the ONE place the schema-vs-bag contract is checked. Returns Result
//! so a malformed bag (missing key, wrong arm — e.g. from schema/pipeline
//! drift) surfaces as a message at Run instead of a panic.
//!
//! Putting this in its own file makes the design claim literally true:
//! if the value-bag has a runtime failure mode, it lives here and nowhere
//! else. The types file can't fail; construction can't fail (it only reads
//! `'static` schema data). Only the conversion out to core's typed form can.

use super::{DitherChoice, OpInstance, ParamValue};
use pixelizer_core::{DitherConfig, Operation};

#[derive(Debug)]
pub struct BuildError {
    pub op: String,
    pub detail: String,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "couldn't build '{}': {}", self.op, self.detail)
    }
}

impl OpInstance {
    /// Read a Num param and narrow to u32 (for the unsigned-int core fields).
    fn u32_field(&self, key: &str) -> Result<u32, BuildError> {
        let n = self
            .values
            .get(key)
            .and_then(ParamValue::as_num)
            .ok_or_else(|| self.miss(key, "expected a number"))?;
        Ok(n.round().max(0.0) as u32)
    }

    /// Read a Num param as f32 (for the float core fields).
    fn f32_field(&self, key: &str) -> Result<f32, BuildError> {
        let n = self
            .values
            .get(key)
            .and_then(ParamValue::as_num)
            .ok_or_else(|| self.miss(key, "expected a number"))?;
        Ok(n as f32)
    }

    fn bool_field(&self, key: &str) -> Result<bool, BuildError> {
        self.values
            .get(key)
            .and_then(ParamValue::as_bool)
            .ok_or_else(|| self.miss(key, "expected a bool"))
    }

    fn miss(&self, key: &str, what: &str) -> BuildError {
        BuildError {
            op: self.tag.clone(),
            detail: format!("param '{key}': {what}"),
        }
    }

    /// Reconstruct the typed core Operation. Called once per op at Run.
    pub fn to_operation(&self) -> Result<Operation, BuildError> {
        Ok(match self.tag.as_str() {
            "downsample" => Operation::Downsample {
                pixel_size: self.u32_field("pixel_size")?,
            },
            "upscale" => Operation::Upscale {
                factor: self.u32_field("factor")?,
            },
            "posterize" => Operation::Posterize {
                levels: self.u32_field("levels")?,
            },
            "blur" => Operation::Blur {
                sigma: self.f32_field("sigma")?,
            },
            "normalize" => Operation::Normalize {
                low: self.f32_field("low")?,
                high: self.f32_field("high")?,
            },
            "resize" => Operation::PixelizerResize {
                exact: self.bool_field("exact")?,
                max_size: self.u32_field("max_size")?,
                width: self.u32_field("width")?,
                height: self.u32_field("height")?,
            },
            "saturation" => Operation::Saturation {
                factor: self.f32_field("factor")?,
            },
            "contrast" => Operation::Contrast {
                factor: self.f32_field("factor")?,
            },
            "palette_map" => {
                let colors = match self.values.get("palette") {
                    Some(ParamValue::Palette(c)) => c.clone(),
                    _ => return Err(self.miss("palette", "expected a palette")),
                };
                let dither = match self.values.get("dither") {
                    Some(ParamValue::Dither(choice)) => {
                        choice.as_ref().map(DitherChoice::to_config).transpose()?
                    }
                    // Absent dither key = no dithering, not an error.
                    None => None,
                    _ => return Err(self.miss("dither", "expected a dither value")),
                };
                let preserve_alpha = match self.values.get("alpha") {
                    Some(ParamValue::Bool(true)) => Some(true),
                    Some(ParamValue::Bool(false)) | None => None,
                    _ => return Err(self.miss("alpha", "expected a bool")),
                };
                Operation::PaletteMap {
                    colors,
                    dither,
                    preserve_alpha,
                }
            }
            "adaptive_palette_map" => {
                let colors = self.u32_field("colors")?;
                let dither = match self.values.get("dither") {
                    Some(ParamValue::Dither(choice)) => {
                        choice.as_ref().map(DitherChoice::to_config).transpose()?
                    }
                    None => None,
                    _ => return Err(self.miss("dither", "expected a dither value")),
                };
                let preserve_alpha = match self.values.get("alpha") {
                    Some(ParamValue::Bool(true)) => Some(true),
                    Some(ParamValue::Bool(false)) | None => None,
                    _ => return Err(self.miss("alpha", "expected a bool")),
                };
                Operation::AdaptivePaletteMap {
                    colors,
                    dither,
                    preserve_alpha,
                }
            }
            other => {
                return Err(BuildError {
                    op: other.to_string(),
                    detail: "unknown operation tag".to_string(),
                });
            }
        })
    }
}

impl DitherChoice {
    fn f32(&self, key: &str) -> Result<f32, BuildError> {
        self.values
            .get(key)
            .and_then(ParamValue::as_num)
            .map(|n| n as f32)
            .ok_or_else(|| BuildError {
                op: format!("dither:{}", self.tag),
                detail: format!("param '{key}': expected a number"),
            })
    }
    fn bool(&self, key: &str) -> Result<bool, BuildError> {
        self.values
            .get(key)
            .and_then(ParamValue::as_bool)
            .ok_or_else(|| BuildError {
                op: format!("dither:{}", self.tag),
                detail: format!("param '{key}': expected a bool"),
            })
    }

    /// Reconstruct the typed DitherConfig. Hand-written (five arms) so the whole
    /// boundary stays serde-free and uniform with the op conversion above.
    pub fn to_config(&self) -> Result<DitherConfig, BuildError> {
        Ok(match self.tag.as_str() {
            "floyd_steinberg" => DitherConfig::FloydSteinberg {
                bleed: self.f32("bleed")?,
                clamp: self.bool("clamp")?,
            },
            "atkinson" => DitherConfig::Atkinson {
                bleed: self.f32("bleed")?,
                clamp: self.bool("clamp")?,
            },
            "jjn" => DitherConfig::Jjn {
                bleed: self.f32("bleed")?,
                clamp: self.bool("clamp")?,
            },
            "bayer4" => DitherConfig::Bayer4 {
                strength: self.f32("strength")?,
            },
            "bayer8" => DitherConfig::Bayer8 {
                strength: self.f32("strength")?,
            },
            other => {
                return Err(BuildError {
                    op: format!("dither:{other}"),
                    detail: "unknown dither tag".to_string(),
                });
            }
        })
    }
}
