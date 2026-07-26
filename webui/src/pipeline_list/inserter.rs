use leptos::{html, portal::Portal, prelude::*};
use leptos_use::{on_click_outside, use_event_listener};
use pixelizer_core::op_schema::all_op_menu;
use web_sys::wasm_bindgen::JsCast;

/// Where and which way the menu opens. When `flip` is false the menu hangs
/// below the strip, anchored by its top at `below`. When true it rises above,
/// anchored by its bottom at `above` (measured from the viewport's bottom edge,
/// since CSS `bottom` is measured that way).
#[derive(Clone, Copy)]
struct DropdownAnchor {
    x: f64,
    w: f64,
    /// Viewport y for the menu's top, in the non-flipped case.
    below: f64,
    /// CSS `bottom` value for the menu, in the flipped case
    /// (= viewport_height - strip_top).
    above: f64,
    flip: bool,
}

/// Menu height ceiling; must match `max_height` in the view. Used to decide
/// whether the menu fits below the strip before flipping it above.
const MENU_MAX_H: f64 = 320.0;

#[component]
pub fn Inserter(
    on_insert: Callback<&'static str>,
    #[prop(into, optional)] always_expanded: Signal<bool>,
) -> impl IntoView {
    let dropdown_ref = NodeRef::<leptos::html::Div>::new();
    let button_ref = NodeRef::<html::Button>::new();
    // The visible strip is this span (the button box is h-0); measure it for a
    // true bottom edge and true top edge.
    let hitbox_ref = NodeRef::<html::Span>::new();
    let max_height = "320px";
    let open: RwSignal<Option<DropdownAnchor>> = RwSignal::new(None);
    let on_click = move |_: leptos::ev::MouseEvent| {
        if open.get_untracked().is_some() {
            open.set(None);
            return;
        }
        // Measure the visible strip (the hitbox span), not the h-0 button box.
        let Some(strip) = hitbox_ref.get_untracked() else {
            return;
        };
        let rect = strip.get_bounding_client_rect();

        let viewport_h = window()
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(800.0);

        // Fit below the strip's bottom edge? If not, flip above its top.
        let space_below = viewport_h - rect.bottom();
        let flip = space_below < MENU_MAX_H;

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
    let expanding_line_class = move || {
        let base = "absolute w-full h-[1px] bg-teal-400 transition-transform\
            duration-300 origin-center pointer-events-none";
        if always_expanded.get() {
            format!("{} scale-x-100", base)
        } else {
            format!("{} scale-x-50 group-hover:scale-x-100", base)
        }
    };
    let expanding_dot_class = move || {
        let base = "absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 \
            flex items-center justify-center rounded-full bg-teal-400 text-white\
            shadow-sm overflow-hidden transition-all duration-300 ease-out\
            pointer-events-none group-focus-visible:ring-2\
            group-focus-visible:ring-teal-500 group-focus-visible:ring-offset-1";
        if always_expanded.get() {
            format!("{} w-7 h-7", base)
        } else {
            format!("{} w-2 h-2 group-hover:w-7 group-hover:h-7", base)
        }
    };
    let plus_class = move || {
        let base = "shrink-0 w-4 h-4 transition-opacity duration-300";
        if always_expanded.get() {
            format!("{} opacity-100", base)
        } else {
            format!("{} opacity-0 group-hover:opacity-100", base)
        }
    };
    view! {
        // The whole strip is the button: the hitbox extends above and below the
        // zero-height line, so the click target is the full width and ~24px tall,
        // which includes the expanded dot sitting at its center.
        <button
            type="button"
            aria-label="Insert operation here"
            class="relative w-full h-0 flex items-center justify-center group z-10 \
                   cursor-pointer focus:outline-none"
            on:click=on_click
            node_ref=button_ref
        >
            // Invisible hitbox (extends above and below); the measured strip.
            <span node_ref=hitbox_ref class="absolute inset-x-0 -top-3 -bottom-3"></span>

            // Animating horizontal line
            <span class=expanding_line_class></span>

            // Expanding dot
            <span class=expanding_dot_class>
                // Plus icon
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"
                    class=plus_class
                >
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4" />
                </svg>
            </span>
        </button>
        {move || open.get().map(|anchor| view! {
            <Portal>
                <div
                    class="fixed z-50 bg-slate-800 border border-slate-700 rounded-md \
                        shadow-xl overflow-y-auto py-1"
                    style:left=format!("{}px", anchor.x)
                    // Non-flipped: anchor top just below the strip.
                    // Flipped: anchor bottom just above the strip, so the menu
                    // grows upward and the browser sizes it to its content.
                    style:top=(!anchor.flip).then(|| format!("{}px", anchor.below))
                    style:bottom=anchor.flip.then(|| format!("{}px", anchor.above))
                    style:width=format!("{}px", anchor.w)
                    style:max-height=max_height
                    node_ref=dropdown_ref
                >
                    // calling all_op_menu() here does not work.
                    {all_op_menu().into_iter().map(|(tag, label)| {
                        view! {
                            <div
                                class="flex flex-col gap-1 px-3 py-2 cursor-pointer hover:bg-slate-700"
                                on:click=move |_| {
                                    on_insert.run(tag);      // tag: &'static str, Copy — no clone
                                    open.set(None);
                                }
                            >
                            {label}
                            </div>
                        }
                    }).collect_view()}
                </div>
            </Portal>
        })}
    }
}
