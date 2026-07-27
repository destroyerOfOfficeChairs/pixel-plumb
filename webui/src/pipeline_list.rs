use crate::{EditPayload, OpRow, op_card, op_instance::default_instance};
use leptos::prelude::*;
use leptos_use::use_event_listener;
use op_card::OpCard;
mod add_op;
mod inserter;
use inserter::Inserter;
mod yaml_preview;
use web_sys::wasm_bindgen::JsCast;
use yaml_preview::YamlPreview;

#[derive(Clone, Copy)]
struct DragState {
    id: usize,
    from: usize,
    start_y: f64,
    pointer_y: f64,
    target: usize, // where it'll land, recomputed on move
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
    // Card vertical midpoints (viewport y), snapshotted at pointerdown.
    // Not reactive — read by the pointermove handler to compute the target index.
    let midpoints = StoredValue::new(Vec::<f64>::new());

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

    let move_op = move |id: usize, dir: i32| {
        set_rows.update(|rows| {
            if let Some(i) = rows.iter().position(|r| r.id == id) {
                let j = i as i32 + dir;
                if j >= 0 && (j as usize) < rows.len() {
                    rows.swap(i, j as usize);
                }
            }
        });
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
        let from = rows
            .with_untracked(|rs| rs.iter().position(|r| r.id == id))
            .unwrap_or(0);
        midpoints.set_value(card_midpoints());
        let y = ev.client_y() as f64;
        drag.set(Some(DragState {
            id,
            from,
            start_y: y,   // grab point — frozen
            pointer_y: y, // current — will update on move
            target: from,
        }));
    };

    let reorder = move |from: usize, to: usize| {
        set_rows.update(|rows| {
            if from >= rows.len() || to > rows.len() || from == to {
                return;
            }
            let row = rows.remove(from);
            // After removing, indices above `from` shift down by one.
            let dest = if to > from { to - 1 } else { to };
            rows.insert(dest, row);
        });
    };

    // Update position + target while dragging. Window-level so it fires even when
    // the pointer leaves the card (which it does constantly during a drag).
    let _ = use_event_listener(window(), leptos::ev::pointermove, move |ev| {
        if let Some(mut st) = drag.get_untracked() {
            st.pointer_y = ev.client_y() as f64;
            st.target =
                midpoints.with_value(|m| m.iter().filter(|&&mid| st.pointer_y > mid).count());
            drag.set(Some(st));
        }
    });

    // Commit the reorder and end the drag.
    let _ = use_event_listener(window(), leptos::ev::pointerup, move |_| {
        if let Some(st) = drag.get_untracked() {
            reorder(st.from, st.target);
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
                        let card_offset = Signal::derive(move || {
                            drag.get().filter(|d| d.id == id).map(|d| d.pointer_y - d.start_y)
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
                                offset=card_offset
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

// At pointerdown: snapshot card midpoints (viewport y).
// The pipeline column has a known container; query its card children.
fn card_midpoints() -> Vec<f64> {
    let doc = document();
    let cards = doc.query_selector_all(".op-card-marker").unwrap();
    let v: Vec<f64> = (0..cards.length())
        .filter_map(|i| cards.item(i))
        .filter_map(|n| n.dyn_into::<web_sys::Element>().ok())
        .map(|el| {
            let r = el.get_bounding_client_rect();
            r.top() + r.height() / 2.0
        })
        .collect();
    v
}
