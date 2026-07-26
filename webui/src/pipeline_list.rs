use crate::{EditPayload, OpRow, op_card, op_instance::default_instance};
use leptos::prelude::*;
use op_card::OpCard;
mod add_op;
use add_op::AddOp;
mod inserter;
use inserter::Inserter;
mod yaml_preview;
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
