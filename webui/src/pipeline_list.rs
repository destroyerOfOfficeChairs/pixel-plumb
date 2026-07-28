use crate::{EditPayload, OpRow, op_card, op_instance::default_instance};
use leptos::prelude::*;
use leptos_use::use_event_listener;
use op_card::OpCard;
mod drag;
mod inserter;
use drag::{DragState, card_tops, flip_play, target_index};
use inserter::Inserter;
mod yaml_preview;
use web_sys::wasm_bindgen::JsCast;
use web_sys::wasm_bindgen::prelude::Closure;
use yaml_preview::YamlPreview;

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
        rows.track();
        if let Some((first, skip_id)) = pending_flip.get_value() {
            pending_flip.set_value(None);
            let run = Closure::once_into_js(move || {
                flip_play(first, skip_id);
                // Correct the dragged card's translate now that the DOM has moved,
                // killing the one-frame flash after a reorder.
                if let Some(mut st) = drag.get_untracked() {
                    if let Some(el) = document()
                        .query_selector(&format!(".op-card-id-{}", st.id))
                        .ok()
                        .flatten()
                        .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
                    {
                        let rendered_top = el.get_bounding_client_rect().top();
                        let home_top = rendered_top - st.translate;
                        st.translate = st.pointer_y - st.grab_offset - home_top;
                        drag.set(Some(st));
                    }
                }
            });
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
        let py = ev.client_y() as f64;
        let card_top = document()
            .query_selector(&format!(".op-card-id-{id}"))
            .ok()
            .flatten()
            .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
            .map(|el| el.get_bounding_client_rect().top())
            .unwrap_or(py);
        drag.set(Some(DragState {
            id,
            pointer_y: py,
            grab_offset: py - card_top,
            translate: 0.0,
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

        // Recompute the dragged card's transform so its rendered top sits
        // exactly grab_offset below the pointer. Measure the card's *current*
        // rendered top, back out the translate already applied to get its true
        // home top, then translate from there. Self-correcting: no accumulation,
        // so it can't drift, and the card stays in flow so the gap still opens.
        if let Some(el) = document()
            .query_selector(&format!(".op-card-id-{}", st.id))
            .ok()
            .flatten()
            .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
        {
            let rendered_top = el.get_bounding_client_rect().top();
            let home_top = rendered_top - st.translate;
            st.translate = st.pointer_y - st.grab_offset - home_top;
        }

        drag.set(Some(st));
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
                        // translateY for the dragged card (None for others).
                        let drag_translate = Signal::derive(move || {
                            drag.get().filter(|d| d.id == id).map(|d| d.translate)
                        });
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
                                drag_translate=drag_translate
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
