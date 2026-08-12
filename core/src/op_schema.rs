//! Descriptor schema for operations and their parameters.
//!
//! Small and UI-agnostic by design: describes each parameter well enough that
//! a generic widget can render it, but names nothing Leptos- or DOM-specific.
//! Core stays free of frontend concepts even though the *reason* this module
//! exists is to feed a UI — the typed `Operation` enum doesn't need any of
//! this to run.
//!
//! Split across three files:
//! - this file: the descriptor types, the two accessors, the cross-cutting
//!   test that ties `ParamKind::Dither` back to `DITHER_VARIANTS`.
//! - `tables.rs`: all the `const *_PARAMS` blocks plus `OP_VARIANTS` and
//!   `DITHER_VARIANTS` — the actual data.
//! - `labels.rs`: small string-presentation helpers keyed by tag.

mod labels;
mod tables;

pub use labels::{all_op_menu, label_for_tag};
pub use tables::{DITHER_VARIANTS, OP_VARIANTS};

// ---------------------------------------------------------------------------
// Descriptor types
// ---------------------------------------------------------------------------

/// One tunable parameter of an operation or dither variant.
#[derive(Clone, Copy, Debug)]
pub struct ParamDescriptor {
    /// The struct field name, and the serde key. e.g. "bleed".
    pub key: &'static str,
    /// Human-facing label for the UI. e.g. "Error bleed".
    pub label: &'static str,
    pub kind: ParamKind,
}

/// The type of a parameter, carrying the metadata a widget needs to render it.
/// This is where the range/default info lives — the stuff a serde schema
/// *can't* express because it isn't in the type.
#[derive(Clone, Copy, Debug)]
pub enum ParamKind {
    /// A float, rendered as a slider (your NumberSlider).
    Float {
        default: f32,
        min: f32,
        max: f32,
        step: f32,
    },
    /// An integer, rendered as a slider or stepper.
    Int { default: i64, min: i64, max: i64 },
    /// A bool, rendered as a checkbox/toggle.
    Bool { default: bool },
    /// A fixed palette of colors (the palette-map param). `colors` is the
    /// default starting palette; the UI swaps it via the palette picker.
    Palette { colors: [&'static str; 2] },
    /// The palette-map dither param. ...
    Dither { default_tag: &'static str },
    /// A generic one-of-N choice, rendered as a dropdown. `options` is a list of
    /// (serde tag, human label) pairs; `default_tag` is the initially-selected
    /// tag and must appear in `options`. Unlike `Dither`, the chosen variant
    /// carries no sub-parameters — it's a plain enum value (e.g. mapping space).
    Enum {
        options: &'static [(&'static str, &'static str)],
        default_tag: &'static str,
    },
}

/// One selectable variant (e.g. one dither algorithm), with its parameters.
#[derive(Clone, Debug)]
pub struct VariantDescriptor {
    /// The serde tag value. e.g. "atkinson" — what goes in `algorithm:`.
    pub tag: &'static str,
    /// Human-facing name for the dropdown. e.g. "Atkinson".
    pub label: &'static str,
    /// The parameters this variant exposes.
    pub params: &'static [ParamDescriptor],
}

// ---------------------------------------------------------------------------
// Accessors — a small indirection so callers don't index the slices directly.
// ---------------------------------------------------------------------------

pub fn dither_variants() -> &'static [VariantDescriptor] {
    DITHER_VARIANTS
}

pub fn op_variants() -> &'static [VariantDescriptor] {
    OP_VARIANTS
}

// ---------------------------------------------------------------------------
// Cross-cutting invariant test. Lives here (not in tables.rs) because it
// checks the relationship between a *type* (ParamKind::Dither) and the
// *tables* — its home is the module that unifies both.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `ParamKind::Dither { default_tag }` must name a real variant in
    /// DITHER_VARIANTS. This is the one string that could silently drift from
    /// the table now that the nested-enum taxonomy is gone; the test ties it
    /// back so a typo fails `cargo test` instead of failing at runtime.
    #[test]
    fn dither_default_tags_exist() {
        let known = |tag: &str| DITHER_VARIANTS.iter().any(|v| v.tag == tag);
        for variant in OP_VARIANTS {
            for p in variant.params {
                if let ParamKind::Dither { default_tag } = p.kind {
                    assert!(
                        known(default_tag),
                        "op '{}' param '{}' has default_tag '{}' \
                         with no matching DITHER_VARIANTS entry",
                        variant.tag,
                        p.key,
                        default_tag,
                    );
                }
            }
        }
    }

    /// Every `ParamKind::Enum { options, default_tag }` must have its default_tag
    /// present in its own options list — otherwise the dropdown would open with a
    /// selection that isn't offered. Catches typos at `cargo test` time.
    #[test]
    fn enum_default_tags_are_in_options() {
        for variant in OP_VARIANTS {
            for p in variant.params {
                if let ParamKind::Enum {
                    options,
                    default_tag,
                } = p.kind
                {
                    assert!(
                        options.iter().any(|(tag, _)| *tag == default_tag),
                        "op '{}' param '{}' default_tag '{}' is not in its options",
                        variant.tag,
                        p.key,
                        default_tag,
                    );
                }
            }
        }
    }
}
