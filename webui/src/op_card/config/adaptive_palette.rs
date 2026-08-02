use super::generic_config::BoolWidget;
use super::generic_config::sliders::IntSlider;
use super::palette_map::dither::DitherConfig;
use crate::op_instance::ParamValue;
use crate::{EditPayload, OpRow};
use leptos::prelude::*;

const COLORS_KEY: &str = "colors";
const COLORS_MIN: i64 = 2;
const COLORS_MAX: i64 = 256;
const COLORS_DEFAULT: i64 = 16;

/// Read the `colors` count from the bag (default 16).
fn read_colors(rows: ReadSignal<Vec<OpRow>>, id: usize) -> i64 {
    rows.with(|rs| {
        rs.iter()
            .find(|r| r.id == id)
            .and_then(|r| r.inst.values.get(COLORS_KEY))
            .and_then(ParamValue::as_num)
    })
    .map(|n| n.round() as i64)
    .unwrap_or(COLORS_DEFAULT)
}

/// Config card for the adaptive palette map. It's the palette_map card with the
/// palette-picking widgets (drop zone, dropdown, swatches) replaced by a single
/// "colors" slider — the palette is generated from the image rather than
/// chosen. The preserve-alpha checkbox and dither config are reused unchanged,
/// writing the same "alpha" / "dither" keys the palette_map card does.
pub fn adaptive_palette_config(
    id: usize,
    rows: ReadSignal<Vec<OpRow>>,
    on_edit: Callback<EditPayload>,
) -> AnyView {
    let colors = Signal::derive(move || read_colors(rows, id));
    let on_colors = Callback::new(move |raw: i64| {
        let v = raw.clamp(COLORS_MIN, COLORS_MAX);
        on_edit.run((id, COLORS_KEY.to_string(), ParamValue::Num(v as f64)));
    });

    view! {
        <div class="flex flex-col">
            <IntSlider
                label="colors"
                value=colors
                min=COLORS_MIN max=COLORS_MAX
                on_commit=on_colors
            />
            <BoolWidget id=id rows=rows on_edit=on_edit default=true key="alpha" label="preserve alpha"/>
            <DitherConfig id=id rows=rows on_edit=on_edit/>
        </div>
    }
    .into_any()
}
