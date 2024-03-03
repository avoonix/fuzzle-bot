mod loader;
mod sticker_set;

pub use loader::*;
pub use sticker_set::*;

use itertools::Itertools;
use leptos::*;
use leptos::{html::*, leptos_dom::logging::console_log};
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};
use web_sys::DragEvent;

use crate::{
    callback::TagOperation,
    web::{client::sticker_context::StickerContext, shared::*},
};

#[component]
fn Tagger(
    #[prop(into)] tags: Signal<Vec<String>>,

    #[prop(into)] suggested_tags: Signal<Vec<String>>,

    #[prop(into)] on_click: Callback<TagOperation>,
) -> impl IntoView {
    let combined = move || {
        tags.get()
            .into_iter()
            .map(|tag| (tag.clone(), TagOperation::Untag(tag)))
            .chain(
                suggested_tags
                    .get()
                    .into_iter()
                    .map(|tag| (tag.clone(), TagOperation::Tag(tag))),
            )
            .collect_vec()
    };

    view! {
        tags:
        <ol>
            <For
                each=combined
                key=|tag| tag.0.clone()
                children=move |tag| {
                    let tag_action = tag.1.clone();
                    view! {
                        <li on:click=move |_| {
                            on_click.call(tag_action.clone());
                        }>
                            {match tag.1 {
                                TagOperation::Untag(tag) => format!("remove tag {tag}"),
                                TagOperation::Tag(tag) => format!("add tag {tag}"),
                            }}
                        </li>
                    }
                }
            />

        </ol>
    }
}

#[component]
pub fn Sticker(id: String) -> impl IntoView {
    let sticker_id = id.clone();
    // let sticker_id = move || sticker_id.clone(); // TODO: use signal

    let sticker_context = use_context::<StickerContext>().unwrap();

    let fetch_sticker_infos = create_action(|input: &(String, Option<TagOperation>)| {
        let input = input.clone();
        // async move { fetch_sticker_info(input.clone()).await }
        async move { tag_sticker(input.0, input.1).await }
    });

    let submitted = fetch_sticker_infos.input(); // RwSignal<Option<String>>
    let pending = fetch_sticker_infos.pending(); // ReadSignal<bool>
    let todo_id = fetch_sticker_infos.value(); // RwSignal<Option<Uuid>>

    let sticker_id_clone = sticker_id.clone();

    let (is_over, set_is_over) = create_signal(false);
    let (is_dragging, set_is_dragging) = create_signal(false);

    let handle_dragover = move |ev: DragEvent| {
        ev.prevent_default();
        if let Some(data_transfer) = ev.data_transfer() {
            data_transfer.set_drop_effect("move");
        }
    };

    let sticker_id_clone = sticker_id.clone();
    let handle_dragstart = move |ev: DragEvent| {
        // example dnd js code had dragSrcEl = this;
        set_is_dragging.set(true);
        if let Some(data_transfer) = ev.data_transfer() {
            data_transfer.set_effect_allowed("move");
            data_transfer
                .set_data("sticker_id", &sticker_id_clone)
                .unwrap();
        }
    };

    let navigate = use_navigate();

    let sticker_id_clone = sticker_id.clone();
    let handle_drop = move |ev: DragEvent| {
        ev.stop_propagation();
        ev.prevent_default();
        if let Some(data_transfer) = ev.data_transfer() {
            let data = data_transfer.get_data("sticker_id").unwrap();
            if data != sticker_id_clone {
                console_log(&format!("{data}, {sticker_id_clone}"));
                navigate(
                    &format!("/merge/{data}/{sticker_id_clone}"),
                    Default::default(),
                );
            }
        }
        //        if (dragSrcEl != this) {
        //   dragSrcEl.innerHTML = this.innerHTML;
        //   this.innerHTML = e.dataTransfer.getData('text/html');
        // }
    };
    let c = sticker_context.clone();
    let handle_dragend = move |ev: DragEvent| {
        set_is_dragging.set(false);
        c.emit_dragend();
    };

    // clear all is_over values whenever dragend changes
    create_effect(move |_| {
        set_is_over.set(sticker_context.dragend.get() < 0);
    });

    // watch(move || sticker_context.dragend.get(), move |_,_,_| {
    //     set_is_over.set(false);
    // }, false);

    let sticker_id_clone = sticker_id.clone();
    let sticker_id_clone_2 = sticker_id.clone();

    view! {
        <div
            draggable="true"
            class:over=move || is_over.get()
            class:dragging=move || is_dragging.get()
            class="box"
            on:mouseenter=move |ev| {
                if todo_id.get().is_none() {
                    fetch_sticker_infos.dispatch((sticker_id.clone(), None));
                }
            }

            on:dragenter=move |ev| { set_is_over.set(true) }

            on:dragleave=move |ev| { set_is_over.set(false) }

            on:dragover=move |ev| { handle_dragover(ev) }

            on:dragstart=move |ev| { handle_dragstart(ev) }

            on:drop=move |ev| { handle_drop(ev) }

            on:dragend=move |ev| { handle_dragend(ev) }
        >

            {move || sticker_context.dragend.get()}
            <A href=format!("/sticker/{sticker_id_clone_2}")>open sticker overview</A>
            <img
                src=format!("/files/stickers/{id}")
                alt=format!("sticker {id}")
                title=format!("sticker {id}")
                width="128"
                height="128"
                loading="lazy"
            />
            <p>{move || submitted.get().map(|_| "Submitted...")}</p>
            <p>{move || pending.get().then(|| view! { <Loader/> })}</p>

            {move || match todo_id.get() {
                None => view! { <p>"hover to load..."</p> }.into_view(),
                Some(results) => {
                    match results {
                        Ok(info) => {
                            let sticker_id = sticker_id_clone.clone();
                            let fetch_sticker_infos = fetch_sticker_infos;
                            view! {
                                // TODO: only fetch tags and suggested tags; no similarities
                                <Tagger
                                    on_click=move |tag_action: TagOperation| {
                                        fetch_sticker_infos
                                            .dispatch((sticker_id.clone(), Some(tag_action)));
                                    }

                                    tags=move || info.tags.clone()
                                    suggested_tags=move || info.suggested_tags.clone()
                                />
                            }
                                .into_view()
                        }
                        Err(err) => {
                            view! {
                                // TODO: only fetch tags and suggested tags; no similarities

                                // TODO: only fetch tags and suggested tags; no similarities

                                // TODO: only fetch tags and suggested tags; no similarities

                                <pre class="error">"Server Error: " {err.to_string()}</pre>
                            }
                                .into_view()
                        }
                    }
                }
            }}

            <code>{format!("{todo_id:?}")}</code>
        </div>
    }
}
