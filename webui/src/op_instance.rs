//! The live, editable representation of a pipeline.
//!
//! This is data-source #1: "the row of operations and their current values."
//! It deliberately does NOT reuse core's typed `Operation` enum as the storage
//! shape. An `Operation` is what core needs at `apply` time; storing it live
//! forces a serde bridge between the generic widgets and the typed fields on
//! every edit. Instead we store a `tag` + a bag of `ParamValue`s — the same
//! shape the descriptor schema (`OP_VARIANTS`) already describes — so a widget
//! reads and writes `values[key]` directly, with no closures and no round-trip.
//!
//! The typed `Operation` is reconstructed once, at the Run boundary
//! (`OpInstance::to_operation`), which is the only place the schema-vs-bag
//! contract is actually checked.
//!
//! Split across three files:
//! - this file: the types (`ParamValue`, `DitherChoice`, `OpInstance`) plus
//!   the small inherent helpers on `ParamValue`.
//! - `default.rs`: fresh-instance construction from the schema.
//! - `boundary.rs`: the one place the typed `Operation` is reconstructed,
//!   and the only runtime failure surface (`BuildError`).

mod boundary;
mod default;

pub use default::{default_dither_choice, default_instance};

use pixelizer_core::op_schema::{VariantDescriptor, op_variants};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Value types — one arm per "kind of thing a widget produces". Each arm exists
// because some ParamKind in the schema yields it:
//   Float | Int      -> Num
//   Bool             -> Bool
//   Palette          -> Palette
//   Dither           -> Dither
// ---------------------------------------------------------------------------

/// A leaf value currently being edited. `Num` covers both Float and Int params;
/// the schema's `ParamKind` carries the int-vs-float distinction (range, step,
/// and how to narrow at the boundary), so the value itself needn't re-encode it.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    Num(f64),
    Bool(bool),
    Palette(Vec<String>),
    Dither(Option<DitherChoice>),
    /// A one-of-N choice, holding the selected serde tag (e.g. "oklab"). Backs
    /// `ParamKind::Enum` params like mapping space. Unlike `Dither` it carries
    /// no sub-parameters — just the chosen tag.
    Enum(String),
}

impl ParamValue {
    pub fn as_num(&self) -> Option<f64> {
        match self {
            ParamValue::Num(n) => Some(*n),
            ParamValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParamValue::Bool(b) => Some(*b),
            ParamValue::Num(n) => Some(*n != 0.0),
            _ => None,
        }
    }
    /// The selected tag for an `Enum` value, if this is one.
    pub fn as_enum(&self) -> Option<&str> {
        match self {
            ParamValue::Enum(tag) => Some(tag),
            _ => None,
        }
    }
}

/// A chosen dither variant plus current values for that variant's params.
/// Structurally a mini-instance (tag + scalar bag), but its own type: its bag
/// only ever holds Num/Bool, never a nested Palette or Dither. Saying so keeps
/// the recursion provably two levels deep — exactly as deep as the data is.
#[derive(Clone, Debug, PartialEq)]
pub struct DitherChoice {
    pub tag: String,
    pub values: BTreeMap<String, ParamValue>,
}

/// One operation instance in the live pipeline. Pure data: serializable as-is,
/// convertible to core::Operation at the boundary. No UI concerns (see OpRow
/// for the `id` the keyed <For> needs).
#[derive(Clone, Debug, PartialEq)]
pub struct OpInstance {
    pub tag: String,
    pub values: BTreeMap<String, ParamValue>,
}

// A tiny helper so callers don't reach into op_variants themselves.
#[allow(dead_code)]
pub fn variant_for(tag: &str) -> Option<&'static VariantDescriptor> {
    op_variants().iter().find(|v| v.tag == tag)
}
