use crate::{EditPayload, OpRow, op_card, op_instance::default_instance};
use leptos::prelude::*;
use leptos_use::use_event_listener;
use op_card::OpCard;
mod add_op;
mod inserter;
use inserter::Inserter;
mod yaml_preview;
use web_sys::wasm_bindgen::JsCast;
use web_sys::wasm_bindgen::prelude::Closure;
use yaml_preview::YamlPreview;

// Parse a card's id out of its class list (op-card-id-N). Classes render
// reliably; the data-attribute path did not (get_attribute couldn't see what
// Leptos's attr: set), so the id rides in a class instead.
fn id_from_element(el: &web_sys::Element) -> Option<usize> {
    el.class_list()
        .value()
        .split_whitespace()
        .find_map(|c| c.strip_prefix("op-card-id-").map(str::to_string))
        .and_then(|s| s.parse::<usize>().ok())
}

// Read each card's current top (viewport y), keyed by card id.
fn card_tops() -> std::collections::HashMap<usize, f64> {
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

/// FLIP invert+play. Called *after* the DOM reflects the reorder (from an
/// Effect keyed on `rows`), so new positions are already correct — measure
/// immediately, invert, then release next frame. `skip_id` is the dragged card.
fn flip_play(first: std::collections::HashMap<usize, f64>, skip_id: usize) {
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
                let style = el.style();
                // let _ = style.set_property("transition", "transform 350ms ease");
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

#[derive(Clone, Copy)]
struct DragState {
    id: usize,
    pointer_y: f64,
}

#[component]
pub fn PipelineList(
    rows: ReadSignal<Vec<OpRow>>,
    set_rows: WriteSignal<Vec<OpRow>>,
    on_run: Callback<()>,
    can_run: Signal<bool>,
) -> impl IntoView {
    // Seeded at 1 because App hardcodes the initial row as id 0.
    let next_id = StoredValue::new(1usize);
    let drag: RwSignal<Option<DragState>> = RwSignal::new(None);

    // FLIP: a pending snapshot of card tops taken just before a reorder, plus
    // the dragged card's id to skip. The effect below consumes it after `rows`
    // (and thus the DOM) has updated.
    type FlipPending = (std::collections::HashMap<usize, f64>, usize);
    let pending_flip: StoredValue<Option<FlipPending>> = StoredValue::new(None);

    // Runs after every `rows` change is reflected in the DOM. The effect clears
    // Leptos's flush; the rAF inside clears the browser's layout reflow (so
    // getBoundingClientRect reads the NEW positions, not the pre-reflow ones).
    Effect::new(move |_| {
        rows.track(); // re-run whenever the list changes
        if let Some((first, skip_id)) = pending_flip.get_value() {
            pending_flip.set_value(None);
            let run = Closure::once_into_js(move || flip_play(first, skip_id));
            let _ = window().request_animation_frame(run.unchecked_ref());
        }
    });

    // Insert `tag` before the row with `before_id`; None appends.
    let insert_op = Callback::new(move |(before_id, tag): (Option<usize>, &'static str)| {
        let Some(inst) = default_instance(tag) else {
            return;
        };
        let id = next_id.get_value();
        next_id.set_value(id + 1);
        set_rows.update(|rows| {
            let pos = before_id
                .and_then(|bid| rows.iter().position(|r| r.id == bid))
                .unwrap_or(rows.len());
            rows.insert(pos, OpRow { id, inst });
        });
    });

    let reorder = move |from: usize, to: usize| {
        set_rows.update(|rows| {
            if from >= rows.len() {
                return;
            }
            let row = rows.remove(from);
            let to = to.min(rows.len());
            rows.insert(to, row);
        });
    };

    let move_op = move |id: usize, dir: i32| {
        let cur = rows.with_untracked(|rs| rs.iter().position(|r| r.id == id));
        let Some(cur) = cur else { return };
        let len = rows.with_untracked(|rs| rs.len());
        let target = cur as i32 + dir;
        if target < 0 || target as usize >= len {
            return;
        }
        let target = target as usize;
        pending_flip.set_value(Some((card_tops(), usize::MAX)));
        reorder(cur, target);
    };

    let remove_op = move |id: usize| {
        set_rows.update(|rows| rows.retain(|r| r.id != id));
    };

    // The single write path for every param edit.
    // A Config emits (id, key, value), which gets dropped into that instance's bag.
    let edit_op = Callback::new(move |(id, key, value): EditPayload| {
        set_rows.update(|rows| {
            if let Some(r) = rows.iter_mut().find(|r| r.id == id) {
                r.inst.values.insert(key, value);
            }
        });
    });

    let start_drag = move |id: usize, ev: leptos::ev::PointerEvent| {
        drag.set(Some(DragState {
            id,
            pointer_y: ev.client_y() as f64,
        }));
    };

    // Live reorder: as the pointer moves, if the dragged card belongs in a
    // different slot than it currently occupies, move it there now. The list
    // reshuffles under the pointer in real time. Window-level so it fires even
    // when the pointer leaves the card (which it does constantly during a drag).
    let _ = use_event_listener(window(), leptos::ev::pointermove, move |ev| {
        let Some(mut st) = drag.get_untracked() else {
            return;
        };
        st.pointer_y = ev.client_y() as f64;
        drag.set(Some(st));

        // Current index of the dragged card (tracked by id — index changes as
        // the list reorders, so we resolve it fresh every move).
        let Some(cur) = rows.with_untracked(|rs| rs.iter().position(|r| r.id == st.id)) else {
            return;
        };

        let desired = target_index(st.pointer_y, st.id);

        if desired != cur {
            // FLIP First: snapshot positions, stash them, then reorder. The
            // effect above fires flip_play once the DOM reflects the change.
            pending_flip.set_value(Some((card_tops(), st.id)));
            reorder(cur, desired);
        }
    });

    // End the drag. The reorder already happened live, so there's nothing to
    // commit here — just clear the state.
    let _ = use_event_listener(window(), leptos::ev::pointerup, move |_| {
        if drag.get_untracked().is_some() {
            drag.set(None);
        }
    });

    view! {
        <div class="w-[28rem] p-4 flex flex-col gap-3">
            <h3 class="text-lg font-bold text-teal-300">"Pipeline"</h3>
            <div class="flex flex-col gap-3">
                <For
                    each=move || rows.get()
                    key=|r| r.id
                    children=move |r| {
                        let id = r.id;
                        view! {
                            <Inserter
                                on_insert=Callback::new(move |tag| insert_op.run((Some(id), tag)))
                            />
                            <OpCard
                                id=id
                                tag=r.inst.tag.clone()
                                rows=rows
                                on_move=Callback::new(move |dir: i32| move_op(id, dir))
                                on_remove=Callback::new(move |_| remove_op(id))
                                on_edit=edit_op
                                on_drag_start=Callback::new(move |ev| start_drag(id, ev))
                                is_dragging=Signal::derive(move || {
                                    drag.get().map(|d| d.id == id).unwrap_or(false)
                                })
                            />
                        }
                    }
                />
                // trailing inserter (append):
                <Inserter
                    on_insert=Callback::new(move |tag| insert_op.run((None, tag)))
                    always_expanded=true
                />
            </div>

            // ---- Run pipeline button ----
            <button
                class="bg-teal-600 hover:bg-teal-500 disabled:bg-slate-700 disabled:text-slate-500 text-white font-bold rounded-md px-4 py-2"
                prop:disabled=move || !can_run.get()
                on:click=move |_| on_run.run(())
            >
                "Run pipeline"
            </button>

            <YamlPreview rows=rows />
        </div>
    }
}

fn target_index(pointer_y: f64, dragged_id: usize) -> usize {
    let doc = document();
    let cards = doc.query_selector_all(".op-card-marker").unwrap();
    (0..cards.length())
        .filter_map(|i| cards.item(i))
        .filter_map(|n| n.dyn_into::<web_sys::Element>().ok())
        .filter(|el| {
            // skip the dragged card's own node
            id_from_element(el) != Some(dragged_id)
        })
        .map(|el| {
            let r = el.get_bounding_client_rect();
            r.top() + r.height() / 2.0
        })
        .filter(|&mid| pointer_y > mid)
        .count()
}
