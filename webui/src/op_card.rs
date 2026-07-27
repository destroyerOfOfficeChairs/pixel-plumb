mod config;
use crate::{EditPayload, OpRow};
use config::op_config_view;
use leptos::html;
use leptos::prelude::*;
use leptos_use::{UseElementSizeReturn, use_element_size};
use pixelizer_core::op_schema::label_for_tag;

// ---- OpCard: one card. Top bar + collapsible animated settings area. ----
#[component]
pub fn OpCard(
    id: usize,
    tag: String,
    rows: ReadSignal<Vec<OpRow>>,
    on_move: Callback<i32>,
    on_remove: Callback<()>,
    on_edit: Callback<EditPayload>,
    #[prop(into)] on_drag_start: Callback<leptos::ev::PointerEvent>,
    #[prop(into)] offset: Signal<Option<f64>>,
) -> impl IntoView {
    let (open, set_open) = signal(true);

    // A handle to the settings-content div, to measure its height.
    let content_ref: NodeRef<html::Div> = NodeRef::new();

    let label = label_for_tag(&tag);

    let UseElementSizeReturn { height, .. } = use_element_size(content_ref);

    let max_height = move || {
        if open.get() {
            format!("{}px", height.get())
        } else {
            "0px".to_string()
        }
    };

    let tag_for_config = tag.clone();

    view! {
        <div class="op-card-marker rounded-lg border border-slate-800 bg-slate-800/30 overflow-hidden"
            style:transform=move || offset.get().map(|dy| format!("translateY({dy}px)")).unwrap_or_default()
            style:pointer-events=move || if offset.get().is_some() { "none" } else { "auto" }
            style:z-index=move || if offset.get().is_some() { "50" } else { "auto" }
        >
            // ---- Top bar (will become the drag handle in M2) ----
            <div class="flex items-center gap-2 px-3 py-2 bg-slate-800/50 border-b border-slate-800 cursor-grab select-none"
                on:pointerdown=move |ev| {
                    ev.prevent_default();      // stop text selection during drag
                    on_drag_start.run(ev);
                }
            >
                // Show/hide toggle — NOT part of the future drag handle.
                <button
                    class="text-slate-500 hover:text-teal-300 px-1"
                    on:click=move |ev| {
                        ev.stop_propagation();
                        set_open.update(|o| *o = !*o);
                    }
                >
                    {move || if open.get() { "▾" } else { "▸" }}
                </button>
                <span class="font-bold text-teal-300 text-sm">{label}</span>
                <button
                    class="ml-auto text-slate-500 hover:text-teal-300 px-1"
                    on:click=move |_| on_move.run(-1)
                >"↑"</button>
                <button
                    class="text-slate-500 hover:text-teal-300 px-1"
                    on:click=move |_| on_move.run(1)
                >"↓"</button>
                <button
                    class="text-slate-500 hover:text-red-400 text-lg leading-none px-1"
                    on:click=move |_| on_remove.run(())
                    on:pointerdown=move |ev| ev.stop_propagation()
                >"×"</button>
            </div>

            // ---- Collapsible settings area ----
            <div
                class="overflow-hidden transition-[max-height] duration-200 ease-in-out"
                style:max-height=max_height
            >
                <div node_ref=content_ref>
                    <div class="p-3">
                        {op_config_view(id, &tag_for_config, rows, on_edit)}
                    </div>
                </div>
            </div>
        </div>
    }
}
