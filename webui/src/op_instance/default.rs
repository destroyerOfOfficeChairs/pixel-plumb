//! Construction from the schema — the defaults live in `OP_VARIANTS`, so
//! building a fresh instance means reading them, never hard-coding them here.

use super::{DitherChoice, OpInstance, ParamValue};
use pixelizer_core::op_schema::{ParamKind, op_variants};

/// The default `ParamValue` for one descriptor param, straight from its kind.
fn default_value(kind: ParamKind) -> ParamValue {
    match kind {
        ParamKind::Float { default, .. } => ParamValue::Num(default as f64),
        ParamKind::Int { default, .. } => ParamValue::Num(default as f64),
        ParamKind::Bool { default } => ParamValue::Bool(default),
        ParamKind::Palette { colors } => {
            ParamValue::Palette(colors.iter().map(|s| s.to_string()).collect())
        }
        // A fresh palette-map starts with dithering OFF. `default_tag` is the
        // variant used when the user later switches it on (handled in the UI),
        // not a reason to start in the Some(_) state.
        ParamKind::Dither { .. } => ParamValue::Dither(None),
        ParamKind::Enum { default_tag, .. } => ParamValue::Enum(default_tag.to_string()),
    }
}

/// Build a fresh, default-valued instance for the op named by `tag`.
/// Returns None for a tag not in OP_VARIANTS.
pub fn default_instance(tag: &str) -> Option<OpInstance> {
    let variant = op_variants().iter().find(|v| v.tag == tag)?;
    let values = variant
        .params
        .iter()
        .map(|p| (p.key.to_string(), default_value(p.kind)))
        .collect();
    Some(OpInstance {
        tag: tag.to_string(),
        values,
    })
}

/// Build a default DitherChoice for a dither variant tag (used when the user
/// switches dithering on or picks a different algorithm). None for unknown tag.
pub fn default_dither_choice(tag: &str) -> Option<DitherChoice> {
    let variant = pixelizer_core::op_schema::dither_variants()
        .iter()
        .find(|v| v.tag == tag)?;
    let values = variant
        .params
        .iter()
        .map(|p| (p.key.to_string(), default_value(p.kind)))
        .collect();
    Some(DitherChoice {
        tag: tag.to_string(),
        values,
    })
}
