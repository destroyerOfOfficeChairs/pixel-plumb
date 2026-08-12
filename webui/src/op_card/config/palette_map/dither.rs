use crate::op_card::config::generic_config::sliders::{FloatSlider, IntSlider, decimals_for_step};
use crate::op_instance::{DitherChoice, ParamValue, default_dither_choice};
use crate::{EditPayload, OpRow};
use leptos::prelude::*;
use pixelizer_core::op_schema::{ParamDescriptor, ParamKind, dither_variants};

const DITHER_KEY: &str = "dither";

// ----------------------------------------------------------------------------
// Reading the nested dither state directly from the value bag.
// ----------------------------------------------------------------------------

/// The live DitherChoice for this op, if dithering is on. Reads the bag; no
/// serialization anywhere.
fn read_choice(rows: ReadSignal<Vec<OpRow>>, id: usize) -> Option<DitherChoice> {
    rows.with(|rs| {
        rs.iter()
            .find(|r| r.id == id)
            .and_then(|r| r.inst.values.get(DITHER_KEY))
            .and_then(|v| match v {
                ParamValue::Dither(choice) => choice.clone(),
                _ => None,
            })
    })
}

/// The current variant tag, or None when dithering is off.
fn current_tag(rows: ReadSignal<Vec<OpRow>>, id: usize) -> Option<String> {
    read_choice(rows, id).map(|c| c.tag)
}

/// Commit a new dither state (Some choice / None) as one value under "dither".
fn commit(on_edit: Callback<EditPayload>, id: usize, choice: Option<DitherChoice>) {
    on_edit.run((id, DITHER_KEY.to_string(), ParamValue::Dither(choice)));
}

// ----------------------------------------------------------------------------
// Param sliders — read a scalar out of the choice's bag, write the whole
// updated choice back. Because a commit replaces the entire DitherChoice, each
// edit clones the current choice, mutates one key, and sends it.
// ----------------------------------------------------------------------------

fn dither_param_widget(
    id: usize,
    rows: ReadSignal<Vec<OpRow>>,
    on_edit: Callback<EditPayload>,
    p: &'static ParamDescriptor,
) -> AnyView {
    let key = p.key;
    let kind = p.kind;

    // Shared: produce a new choice with `key` set to `value`, then commit.
    let commit_field = move |value: ParamValue| {
        if let Some(mut choice) = read_choice(rows, id) {
            choice.values.insert(key.to_string(), value);
            commit(on_edit, id, Some(choice));
        }
    };

    match kind {
        ParamKind::Float {
            default,
            min,
            max,
            step,
        } => {
            let value = Signal::derive(move || {
                read_choice(rows, id)
                    .and_then(|c| c.values.get(key).and_then(ParamValue::as_num))
                    .unwrap_or(default as f64)
            });
            let on_commit = Callback::new(move |raw: f64| commit_field(ParamValue::Num(raw)));
            view! {
                <FloatSlider
                    label=p.label
                    value=value
                    min=min as f64 max=max as f64 step=step as f64
                    decimals=decimals_for_step(step as f64)
                    on_commit=on_commit
                />
            }
            .into_any()
        }

        ParamKind::Int { default, min, max } => {
            let value = Signal::derive(move || {
                read_choice(rows, id)
                    .and_then(|c| c.values.get(key).and_then(ParamValue::as_num))
                    .map(|n| n.round() as i64)
                    .unwrap_or(default)
            });
            let on_commit = Callback::new(move |raw: i64| {
                commit_field(ParamValue::Num(raw.clamp(min, max) as f64))
            });
            view! {
                <IntSlider
                    label=p.label
                    value=value
                    min=min max=max
                    on_commit=on_commit
                />
            }
            .into_any()
        }

        ParamKind::Bool { default } => {
            let value = Signal::derive(move || {
                read_choice(rows, id)
                    .and_then(|c| c.values.get(key).and_then(ParamValue::as_bool))
                    .unwrap_or(default)
            });
            let on_commit =
                Callback::new(move |checked: bool| commit_field(ParamValue::Bool(checked)));
            view! {
                <label class="flex items-center gap-2 text-xs text-slate-400 px-3 pb-2">
                    <input
                        type="checkbox"
                        prop:checked=move || value.get()
                        on:change=move |ev| on_commit.run(event_target_checked(&ev))
                    />
                    {p.label}
                </label>
            }
            .into_any()
        }

        // A dither variant's params are always scalars; these shouldn't occur.
        ParamKind::Palette { .. } | ParamKind::Dither { .. } | ParamKind::Enum { .. } => view! {
            <p class="text-xs text-red-400 px-3">"non-scalar dither param"</p>
        }
        .into_any(),
    }
}

// ----------------------------------------------------------------------------
// The component
// ----------------------------------------------------------------------------

/// Dither config, a child of the PaletteMap card. On/off toggle drives
/// Some/None; when on, a variant dropdown plus that variant's params.
#[component]
pub fn DitherConfig(
    id: usize,
    rows: ReadSignal<Vec<OpRow>>,
    on_edit: Callback<EditPayload>,
) -> impl IntoView {
    let enabled = Signal::derive(move || current_tag(rows, id).is_some());

    // Toggle on -> default choice (first variant). Toggle off -> None.
    let on_toggle = move |ev: leptos::ev::Event| {
        let choice = if event_target_checked(&ev) {
            dither_variants()
                .first()
                .and_then(|v| default_dither_choice(v.tag))
        } else {
            None
        };
        commit(on_edit, id, choice);
    };

    // Switch variant -> a fresh default choice for the chosen tag.
    let on_variant = move |ev: leptos::ev::Event| {
        let tag = event_target_value(&ev);
        commit(on_edit, id, default_dither_choice(&tag));
    };

    // The selected variant descriptor drives the param sliders.
    let selected = Signal::derive(move || {
        current_tag(rows, id).and_then(|tag| dither_variants().iter().find(|v| v.tag == tag))
    });

    view! {
        <div class="border-t border-slate-800 mt-2 pt-2">
            <label class="flex items-center gap-2 text-xs font-bold text-teal-300 px-3 pb-1">
                <input
                    type="checkbox"
                    prop:checked=move || enabled.get()
                    on:change=on_toggle
                />
                "Dither"
            </label>

            {move || enabled.get().then(|| view! {
                <div class="px-3 pb-2">
                    <select
                        class="bg-slate-900 border border-slate-700 rounded-md text-sm \
                               text-slate-200 p-2 w-full"
                        on:change=on_variant
                    >
                        {dither_variants().iter().map(|v| {
                            let tag = v.tag;
                            view! {
                                <option
                                    value=tag
                                    selected=move || current_tag(rows, id).as_deref() == Some(tag)
                                >
                                    {v.label}
                                </option>
                            }
                        }).collect_view()}
                    </select>
                </div>

                {move || selected.get().map(|v| {
                    v.params
                        .iter()
                        .map(|p| dither_param_widget(id, rows, on_edit, p))
                        .collect_view()
                })}
            })}
        </div>
    }
}
