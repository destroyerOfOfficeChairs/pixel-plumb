mod generic_config;
mod palette_map;
mod resize;

use generic_config::generic_op_config;
use palette_map::palette_map_config;
use resize::resize_config;

use crate::{EditPayload, OpRow};
use leptos::prelude::*;

/// Dispatch a config view by op tag. The scalar ops go through the generic
/// descriptor-driven renderer; palette_map and resize are the special cases
/// (palette_map's params aren't scalars; resize has a two-mode layout that the
/// generic renderer can't express). No serde helpers live here anymore — the
/// store is a value bag the widgets read and write directly.
pub fn op_config_view(
    id: usize,
    tag: &str,
    rows: ReadSignal<Vec<OpRow>>,
    on_edit: Callback<EditPayload>,
) -> AnyView {
    match tag {
        "palette_map" => palette_map_config(id, rows, on_edit),
        "resize" => resize_config(id, rows, on_edit),
        _ => generic_op_config(id, tag, rows, on_edit),
    }
}
