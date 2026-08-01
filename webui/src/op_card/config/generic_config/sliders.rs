use leptos::prelude::*;

// ----------------------------------------------------------------------------
// Precision helper: decimals implied by a step value.
//   0.1  -> 1
//   0.01 -> 2
//   1.0  -> 0   (shouldn't be used for floats, but safe)
// Kept here (webui) rather than core: it's a display concern. Core owns `step`;
// the UI decides how to render it.
// ----------------------------------------------------------------------------
pub fn decimals_for_step(step: f64) -> usize {
    if step <= 0.0 || step >= 1.0 {
        return 0;
    }
    // The step might be slightly off (e.g. 0.01_f32 -> 0.00999... as f64),
    // so nudge the log slightly before ceiling to absorb that noise.
    (-step.log10() - 1e-6).ceil() as usize
}

// ----------------------------------------------------------------------------
// IntSlider — integer-valued. Number box + range, both in i64.
// ----------------------------------------------------------------------------
#[component]
pub fn IntSlider(
    label: &'static str,
    value: Signal<i64>,
    min: i64,
    max: i64,
    /// Called with the parsed i64. Caller stores it.
    on_commit: Callback<i64>,
) -> impl IntoView {
    let shown = move || value.get().to_string();

    // Handler used by both change (typing/blur) and input (arrow clicks, drag).
    let commit = move |ev: leptos::ev::Event| {
        let raw: i64 = event_target_value(&ev).parse().unwrap_or(min);
        on_commit.run(raw.clamp(min, max));
    };

    view! {
        <label class="text-xs text-slate-400 block p-3">
            {label}": "
            <input
                type="number"
                class="w-20 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm text-slate-200"
                min=min as f64 max=max as f64 step=1.0
                prop:value=shown
                on:change=commit
                on:focus=move |ev| {
                    let target: web_sys::HtmlInputElement = event_target(&ev);
                    let _ = target.select();
                }
            />
            <input
                type="range"
                min=min as f64 max=max as f64 step=1.0
                prop:value=shown
                class="w-full accent-teal-500"
                on:input=move |ev| {
                    let raw: i64 = event_target_value(&ev)
                        .parse::<f64>()
                        .map(|f| f.round() as i64)
                        .unwrap_or(min);
                    on_commit.run(raw.clamp(min, max));
                }
            />
        </label>
    }
}

fn snap(raw: f64, min: f64, max: f64, step: f64, decimals: usize) -> f64 {
    let clamped = raw.clamp(min, max);
    if step <= 0.0 {
        return clamped;
    }
    let snapped = (clamped / step).round() * step;
    // Round-trip through the display format so committed value == displayed value.
    // Kills the "0.30000000000000004" flash.
    format!("{:.*}", decimals, snapped)
        .parse()
        .unwrap_or(snapped)
}

// ----------------------------------------------------------------------------
// FloatSlider — float-valued. Number box + range, in f64, with explicit
// display precision (from decimals_for_step at the call site).
// ----------------------------------------------------------------------------
#[component]
pub fn FloatSlider(
    label: &'static str,
    value: Signal<f64>,
    min: f64,
    max: f64,
    step: f64,
    /// How many decimal places to display.
    decimals: usize,
    /// Called with the parsed f64. Caller stores it.
    on_commit: Callback<f64>,
) -> impl IntoView {
    let shown = move || format!("{:.*}", decimals, value.get());

    // The schema's step comes in as f32-widened-to-f64, so 0.01 arrives as
    // ~0.00999999... . Passing that raw to the DOM makes the browser display
    // an extra decimal (its internal representation follows the step). Round
    // it through the display format so the DOM sees exactly "0.01".
    let clean_step: f64 = format!("{:.*}", decimals, step).parse().unwrap_or(step);

    // Handler used by both change (typing/blur) and input (arrow clicks, drag).
    let commit = move |ev: leptos::ev::Event| {
        let raw: f64 = event_target_value(&ev).parse().unwrap_or(min);
        on_commit.run(snap(raw, min, max, step, decimals));
    };

    view! {
        <label class="text-xs text-slate-400 block p-3">
            {label}": "
            <input
                type="number"
                class="w-20 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm text-slate-200"
                min=min max=max step=clean_step
                prop:value=shown
                on:change=commit
                on:focus=move |ev| {
                    let target: web_sys::HtmlInputElement = event_target(&ev);
                    let _ = target.select();
                }
            />
            <input
                type="range"
                min=min max=max step=clean_step
                prop:value=shown
                class="w-full accent-teal-500"
                on:input=move |ev| {
                    let raw: f64 = event_target_value(&ev).parse().unwrap_or(min);
                    on_commit.run(snap(raw, min, max, step, decimals));
                }
            />
        </label>
    }
}
