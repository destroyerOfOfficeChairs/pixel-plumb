use crate::Palettes;
use leptos::{html, portal::Portal, prelude::*};
use leptos_use::{on_click_outside, use_event_listener};
use web_sys::wasm_bindgen::JsCast;

/// Where and which way the menu opens. Not flipped: hangs below, anchored by
/// its top at `below`. Flipped: rises above, anchored by its bottom at `above`
/// (a CSS `bottom` value, measured from the viewport's bottom edge).
#[derive(Clone, Copy)]
struct DropdownAnchor {
    x: f64,
    w: f64,
    below: f64,
    above: f64,
    flip: bool,
}

/// Menu height ceiling; must match `max_height` in the view. Used to decide
/// whether the menu fits below the button before flipping it above.
const MENU_MAX_H: f64 = 320.0;

#[component]
pub fn PaletteDropdown(
    palettes: RwSignal<Palettes>,
    on_select: Callback<Vec<String>>,
) -> impl IntoView {
    let dropdown_ref = NodeRef::<leptos::html::Div>::new();
    let button_ref = NodeRef::<html::Button>::new();
    let max_height = "320px";
    let open: RwSignal<Option<DropdownAnchor>> = RwSignal::new(None);
    let on_click = move |_: leptos::ev::MouseEvent| {
        if open.get_untracked().is_some() {
            open.set(None);
            return;
        }
        // Measure the button itself, not the event target — a click on the
        // button's text child would otherwise measure the text node.
        let Some(btn) = button_ref.get_untracked() else {
            return;
        };
        let rect = btn.get_bounding_client_rect();
        let viewport_h = window()
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(800.0);
        // Fit below the button's bottom edge? If not, flip above its top.
        let flip = viewport_h - rect.bottom() < MENU_MAX_H;
        open.set(Some(DropdownAnchor {
            x: rect.x(),
            w: rect.width(),
            below: rect.bottom(),
            above: viewport_h - rect.top(),
            flip,
        }));
    };
    let _ = use_event_listener(window(), leptos::ev::scroll, move |_| open.set(None));
    let _ = on_click_outside(dropdown_ref, move |ev| {
        if let Some(btn) = button_ref.get_untracked() {
            if let Some(target) = ev.target() {
                if let Ok(node) = target.dyn_into::<web_sys::Node>() {
                    if btn.contains(Some(&node)) {
                        return;
                    }
                }
            }
        }
        open.set(None);
    });
    view! {
        <button
            class="bg-teal-600 hover:bg-teal-500 text-white font-bold rounded-md px-4 py-2"
            on:click=on_click
            node_ref=button_ref
        >
            "Select Palette"
        </button>
        {move || open.get().map(|anchor| view! {
            <Portal>
                <div
                    class="fixed z-50 bg-slate-800 border border-slate-700 rounded-md \
                        shadow-xl overflow-y-auto py-1"
                    style:left=format!("{}px", anchor.x)
                    style:top=(!anchor.flip).then(|| format!("{}px", anchor.below))
                    style:bottom=anchor.flip.then(|| format!("{}px", anchor.above))
                    style:width=format!("{}px", anchor.w)
                    style:max-height=max_height
                    node_ref=dropdown_ref
                >
                    // rows here
                    {move || palettes.with(|p| {
                        p.palettes.iter().map(|(name, colors)| {
                            let colors = colors.clone();
                            view! {
                                <div
                                    class="flex flex-col gap-1 px-3 py-2 cursor-pointer hover:bg-slate-700"
                                    on:click=move |_| {
                                        on_select.run(colors.clone());
                                        open.set(None);
                                    }
                                >
                                    <span class="text-sm text-slate-200 flex-1 truncate">{name.clone()}</span>
                                    // swatch strip goes here
                                    <div class="flex flex-wrap w-full">
                                        {colors.iter().map(|c| view! {
                                            <div class="w-2 h-2" style:background-color=c.clone()/>
                                        }).collect_view()}
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    })}
                </div>
            </Portal>
        })}
    }
}
