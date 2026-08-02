mod color_picker;
pub(super) mod dither;
mod dropdown;
mod dropzone;
mod swatches;

use super::generic_config::BoolWidget;
use crate::op_instance::ParamValue;
use crate::{EditPayload, OpRow, Palettes};
use dither::DitherConfig;
use dropdown::PaletteDropdown;
use dropzone::PaletteDropZone;
use leptos::prelude::*;
use swatches::Swatches;

const PALETTE_KEY: &'static str = "palette";
pub const MAX_PALETTE_COLORS: usize = 256;

pub fn palette_map_config(
    id: usize,
    rows: ReadSignal<Vec<OpRow>>,
    on_edit: Callback<EditPayload>,
) -> AnyView {
    let preloaded_palettes =
        use_context::<RwSignal<Palettes>>().expect("You forgot to provide palettes.");

    let on_load = Callback::new(move |(name, colors): (String, Vec<String>)| {
        preloaded_palettes.update(|p| {
            // Re-uploading a same-named file replaces the old entry
            p.palettes.retain(|(n, _)| *n != name);
            p.palettes.push((name.clone(), colors.clone()));
            p.palettes.sort_by(|a, b| a.0.cmp(&b.0));
        });
        // Auto-select the upload
        on_edit.run((id, PALETTE_KEY.to_string(), ParamValue::Palette(colors)));
    });

    let on_select = Callback::new(move |colors: Vec<String>| {
        on_edit.run((id, PALETTE_KEY.to_string(), ParamValue::Palette(colors)));
    });

    view! {
        <div class="flex flex-col">
            <PaletteDropZone on_load=on_load />
            <PaletteDropdown palettes=preloaded_palettes on_select=on_select />
            <Swatches id=id rows=rows on_edit=on_edit palette_key=PALETTE_KEY/>
            // TODO: Remove hardcoded "default=true", "key=alpha", and "label=preserve alpha" in favor of reading from the op_schema
            <BoolWidget id=id rows=rows on_edit=on_edit default=true key="alpha" label="preserve alpha"/>
            <DitherConfig id=id rows=rows on_edit=on_edit/>
        </div>
    }
    .into_any()
}
