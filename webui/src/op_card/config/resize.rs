use crate::op_instance::ParamValue;
use crate::{EditPayload, OpRow};
use leptos::prelude::*;

use super::generic_config::sliders::IntSlider;

// Reused across the card. Defaults match core's default_resize_dim (64) and
// exact defaulting to false.
const DIM_MIN: i64 = 1;
const DIM_MAX: i64 = 4096;

/// Read an int param from the bag as i64, or its default.
fn read_int(rows: ReadSignal<Vec<OpRow>>, id: usize, key: &str, default: i64) -> i64 {
    rows.with(|rs| {
        rs.iter()
            .find(|r| r.id == id)
            .and_then(|r| r.inst.values.get(key))
            .and_then(ParamValue::as_num)
    })
    .map(|n| n.round() as i64)
    .unwrap_or(default)
}

/// Read the `exact` bool from the bag (default false).
fn read_exact(rows: ReadSignal<Vec<OpRow>>, id: usize) -> bool {
    rows.with(|rs| {
        rs.iter()
            .find(|r| r.id == id)
            .and_then(|r| r.inst.values.get("exact"))
            .and_then(ParamValue::as_bool)
    })
    .unwrap_or(false)
}

/// Bespoke config card for the resize op.
///
/// A checkbox ("specify exact resolution") selects between two modes, and the
/// half that isn't active is greyed out:
/// - unchecked (default): one slider, `max_size` — longest-side proportional.
/// - checked: `width` and `height` sliders — exact dimensions.
///
/// Both halves always render (their values persist in the bag either way); only
/// the visual/interactive enabled state changes, so toggling back and forth
/// doesn't lose what you typed.
pub fn resize_config(
    id: usize,
    rows: ReadSignal<Vec<OpRow>>,
    on_edit: Callback<EditPayload>,
) -> AnyView {
    let exact = Signal::derive(move || read_exact(rows, id));

    // ---- max_size (longest-side mode) ----
    let max_size = Signal::derive(move || read_int(rows, id, "max_size", 64));
    let on_max = Callback::new(move |raw: i64| {
        let v = raw.clamp(DIM_MIN, DIM_MAX);
        on_edit.run((id, "max_size".to_string(), ParamValue::Num(v as f64)));
    });

    // ---- width / height (exact mode) ----
    let width = Signal::derive(move || read_int(rows, id, "width", 64));
    let on_width = Callback::new(move |raw: i64| {
        let v = raw.clamp(DIM_MIN, DIM_MAX);
        on_edit.run((id, "width".to_string(), ParamValue::Num(v as f64)));
    });
    let height = Signal::derive(move || read_int(rows, id, "height", 64));
    let on_height = Callback::new(move |raw: i64| {
        let v = raw.clamp(DIM_MIN, DIM_MAX);
        on_edit.run((id, "height".to_string(), ParamValue::Num(v as f64)));
    });

    // ---- the exact toggle ----
    let on_toggle = Callback::new(move |checked: bool| {
        on_edit.run((id, "exact".to_string(), ParamValue::Bool(checked)));
    });

    // Greying: the inactive half is dimmed and non-interactive. A half is active
    // when its mode is selected.
    let longest_cls = move || {
        if exact.get() {
            "opacity-40 pointer-events-none"
        } else {
            ""
        }
    };
    let exact_cls = move || {
        if exact.get() {
            ""
        } else {
            "opacity-40 pointer-events-none"
        }
    };

    view! {
        <div class="flex flex-col gap-2 p-3">
            // Mode toggle
            <label class="flex items-center gap-2 text-xs text-slate-300">
                <input
                    type="checkbox"
                    prop:checked=move || exact.get()
                    on:change=move |ev| on_toggle.run(event_target_checked(&ev))
                />
                "specify exact resolution"
            </label>

            // Longest-side half
            <div class=longest_cls>
                <IntSlider
                    label="max size"
                    value=max_size
                    min=DIM_MIN max=DIM_MAX
                    on_commit=on_max
                />
            </div>

            <div class="h-px bg-slate-800"></div>

            // Exact half
            <div class=exact_cls>
                <IntSlider
                    label="width"
                    value=width
                    min=DIM_MIN max=DIM_MAX
                    on_commit=on_width
                />
                <IntSlider
                    label="height"
                    value=height
                    min=DIM_MIN max=DIM_MAX
                    on_commit=on_height
                />
            </div>
        </div>
    }
    .into_any()
}
