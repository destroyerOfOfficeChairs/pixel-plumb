//! The data. Every `const *_PARAMS: &[ParamDescriptor]`, plus the two
//! variant tables (`OP_VARIANTS`, `DITHER_VARIANTS`) and shared constants
//! they refer to. Read this file when you want to know *what params does
//! blur have*; read the parent `op_schema` when you want to know *what
//! shape is a param*.

use super::{ParamDescriptor, ParamKind, VariantDescriptor};

const DEFAULT_PALETTE: [&str; 2] = ["#000000", "#ffffff"];

// ---------------------------------------------------------------------------
// Per-op / per-dither param blocks
// ---------------------------------------------------------------------------

const DIFFUSE_PARAMS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "bleed",
        label: "Error bleed",
        kind: ParamKind::Float {
            default: 1.0,
            min: 0.0,
            max: 1.0,
            step: 0.05,
        },
    },
    ParamDescriptor {
        key: "clamp",
        label: "Clamp to palette range",
        kind: ParamKind::Bool { default: true },
    },
];

const BAYER_PARAMS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "strength",
    label: "Dither strength",
    kind: ParamKind::Float {
        default: 32.0,
        min: 0.0,
        max: 64.0,
        step: 1.0,
    },
}];

const DOWNSAMPLE_PARAMS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "pixel_size",
    label: "pixel size",
    kind: ParamKind::Int {
        default: 8,
        min: 1,
        max: 64,
    },
}];

const UPSCALE_PARAMS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "factor",
    label: "scale factor",
    kind: ParamKind::Int {
        default: 8,
        min: 1,
        max: 64,
    },
}];

const NORMALIZE_PARAMS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "low",
        label: "low value cutoff",
        kind: ParamKind::Float {
            default: 0.01,
            min: 0.0,
            max: 1.0,
            step: 0.01,
        },
    },
    ParamDescriptor {
        key: "high",
        label: "high value cutoff",
        kind: ParamKind::Float {
            default: 0.99,
            min: 0.0,
            max: 1.0,
            step: 0.01,
        },
    },
];

const BLUR_PARAMS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "sigma",
    label: "sigma",
    kind: ParamKind::Float {
        default: 1.0,
        min: 0.0,
        max: 10.0,
        step: 0.1,
    },
}];

const POSTERIZE_PARAMS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "levels",
    label: "levels",
    kind: ParamKind::Int {
        default: 4,
        min: 2,
        max: 16,
    },
}];

const SATURATION_PARAMS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "factor",
    label: "factor",
    kind: ParamKind::Float {
        default: 1.0,
        min: 0.0,
        max: 2.0,
        step: 0.05,
    },
}];

const CONTRAST_PARAMS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "factor",
    label: "factor",
    kind: ParamKind::Float {
        default: 1.0,
        min: 0.0,
        max: 2.0,
        step: 0.05,
    },
}];

const PALETTE_MAP_PARAMS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "palette",
        label: "palette",
        kind: ParamKind::Palette {
            colors: DEFAULT_PALETTE,
        },
    },
    ParamDescriptor {
        key: "alpha",
        label: "preserve alpha",
        kind: ParamKind::Bool { default: true },
    },
    ParamDescriptor {
        key: "dither",
        label: "dither",
        kind: ParamKind::Dither {
            default_tag: "bayer8",
        },
    },
];

// ---------------------------------------------------------------------------
// The two variant tables
// ---------------------------------------------------------------------------

/// Every dither variant the UI should offer. The `None` (no dithering) case
/// is handled by the UI separately — this table is the `Some(_)` options.
pub const DITHER_VARIANTS: &[VariantDescriptor] = &[
    VariantDescriptor {
        tag: "floyd_steinberg",
        label: "Floyd–Steinberg",
        params: DIFFUSE_PARAMS,
    },
    VariantDescriptor {
        tag: "atkinson",
        label: "Atkinson",
        params: DIFFUSE_PARAMS,
    },
    VariantDescriptor {
        tag: "jjn",
        label: "JJN",
        params: DIFFUSE_PARAMS,
    },
    VariantDescriptor {
        tag: "bayer4",
        label: "Bayer 4×4",
        params: BAYER_PARAMS,
    },
    VariantDescriptor {
        tag: "bayer8",
        label: "Bayer 8×8",
        params: BAYER_PARAMS,
    },
];

pub const OP_VARIANTS: &[VariantDescriptor] = &[
    VariantDescriptor {
        tag: "downsample",
        label: "downsample",
        params: DOWNSAMPLE_PARAMS,
    },
    VariantDescriptor {
        tag: "upscale",
        label: "upscale",
        params: UPSCALE_PARAMS,
    },
    VariantDescriptor {
        tag: "normalize",
        label: "normalize",
        params: NORMALIZE_PARAMS,
    },
    VariantDescriptor {
        tag: "blur",
        label: "blur",
        params: BLUR_PARAMS,
    },
    VariantDescriptor {
        tag: "posterize",
        label: "posterize",
        params: POSTERIZE_PARAMS,
    },
    VariantDescriptor {
        tag: "saturation",
        label: "saturation",
        params: SATURATION_PARAMS,
    },
    VariantDescriptor {
        tag: "contrast",
        label: "contrast",
        params: CONTRAST_PARAMS,
    },
    VariantDescriptor {
        tag: "palette_map",
        label: "palette map",
        params: PALETTE_MAP_PARAMS,
    },
];
