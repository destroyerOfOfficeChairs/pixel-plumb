//! Drag-and-drop machinery for the pipeline list.
//!
//! Pure mechanism: none of this knows what a pipeline or an op is. It operates
//! entirely on the DOM by class — `.op-card-marker` (every card) and
//! `.op-card-id-N` (a specific card) — plus viewport pixel coordinates. That
//! independence is why it lives apart from `PipelineList`, which uses these as
//! a capability rather than containing the logic.
//!
//! Three concerns:
//! - **Identity/measurement**: `id_from_element`, `card_tops`, `target_index`
//!   read card ids and positions out of the live DOM.
//! - **Animation**: `flip_play` runs the FLIP (First-Last-Invert-Play) slide on
//!   the non-dragged cards after a reorder.
//! - **State**: `DragState` is the live drag; the dragged card's `translate`
//!   self-corrects each move so it tracks the cursor without drifting.

use leptos::prelude::*;
use web_sys::wasm_bindgen::JsCast;
use web_sys::wasm_bindgen::prelude::Closure;

/// Parse a card's id out of its class list (`op-card-id-N`). Classes render
/// reliably; the data-attribute path did not (`get_attribute` couldn't see what
/// Leptos's `attr:` set), so the id rides in a class instead.
pub fn id_from_element(el: &web_sys::Element) -> Option<usize> {
    el.class_list()
        .value()
        .split_whitespace()
        .find_map(|c| c.strip_prefix("op-card-id-").map(str::to_string))
        .and_then(|s| s.parse::<usize>().ok())
}

/// Read each card's current top (viewport y), keyed by card id.
pub fn card_tops() -> std::collections::HashMap<usize, f64> {
    let mut map = std::collections::HashMap::new();
    let doc = document();
    let Ok(cards) = doc.query_selector_all(".op-card-marker") else {
        return map;
    };
    for i in 0..cards.length() {
        let Some(el) = cards
            .item(i)
            .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
        else {
            continue;
        };
        if let Some(id) = id_from_element(&el) {
            map.insert(id, el.get_bounding_client_rect().top());
        }
    }
    map
}

/// The slot the pointer sits in: how many *other* cards' midpoints are above it.
/// The dragged card is excluded so it doesn't count its own midpoint. Measured
/// live because the cards move as the list reorders.
pub fn target_index(pointer_y: f64, dragged_id: usize) -> usize {
    let doc = document();
    let cards = doc.query_selector_all(".op-card-marker").unwrap();
    (0..cards.length())
        .filter_map(|i| cards.item(i))
        .filter_map(|n| n.dyn_into::<web_sys::Element>().ok())
        .filter(|el| id_from_element(el) != Some(dragged_id))
        .map(|el| {
            let r = el.get_bounding_client_rect();
            r.top() + r.height() / 2.0
        })
        .filter(|&mid| pointer_y > mid)
        .count()
}

/// FLIP invert+play. Called *after* the DOM reflects the reorder (from an
/// Effect keyed on `rows`), so new positions are already correct — measure
/// immediately, invert, then release next frame. `skip_id` is the dragged card,
/// whose transform is driven reactively to follow the pointer.
pub fn flip_play(first: std::collections::HashMap<usize, f64>, skip_id: usize) {
    let doc = document();
    let Ok(cards) = doc.query_selector_all(".op-card-marker") else {
        return;
    };
    for i in 0..cards.length() {
        let Some(el) = cards
            .item(i)
            .and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
        else {
            continue;
        };
        let Some(id) = id_from_element(&el) else {
            continue;
        };
        if id == skip_id {
            continue;
        }
        let Some(&old_top) = first.get(&id) else {
            continue;
        };
        let new_top = el.get_bounding_client_rect().top();
        let delta = old_top - new_top;
        let style = el.style();
        let _ = style.set_property("transition", "none");
        if delta.abs() < 0.5 {
            let _ = style.set_property("transform", "translateY(0)");
        } else {
            let _ = style.set_property("transform", &format!("translateY({delta}px)"));
        }
    }

    // Release next frame: re-enable the transition and clear the transform so
    // each card glides from its inverted (old) position to its real (new) one.
    let release = Closure::once_into_js(move || {
        let doc = document();
        let Ok(cards) = doc.query_selector_all(".op-card-marker") else {
            return;
        };
        for i in 0..cards.length() {
            if let Some(el) = cards
                .item(i)
                .and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
            {
                // Skip the dragged card — its transform is driven reactively to
                // follow the pointer. Writing here would clobber it and impose
                // the slide transition, making it lag the cursor.
                if id_from_element(&el) == Some(skip_id) {
                    continue;
                }
                let style = el.style();
                let _ = style.set_property(
                    "transition",
                    "transform 380ms cubic-bezier(0.34, 1.56, 0.64, 1)",
                );
                let _ = style.set_property("transform", "translateY(0)");
            }
        }
    });
    let _ = window().request_animation_frame(release.unchecked_ref());
}

/// The live drag. `None` when nothing is being dragged.
#[derive(Clone, Copy)]
pub struct DragState {
    pub id: usize,
    pub pointer_y: f64,
    /// Where on the card the pointer grabbed it (pointer_y - card_top at grab).
    /// Constant for the drag; the card's rendered top should stay this far below
    /// the pointer so the cursor sticks to the grab point.
    pub grab_offset: f64,
    /// The translateY currently applied to the dragged card. Recomputed each
    /// move by measuring the card's real position and backing out this value,
    /// so it self-corrects instead of accumulating drift.
    pub translate: f64,
}
