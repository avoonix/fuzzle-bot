use std::vec;

use itertools::Itertools;
use leptos::html::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};

use crate::{callback::TagOperation, web::shared::*};

#[component]
fn Tagger(
    #[prop(into)] tags: Signal<Vec<String>>,

    #[prop(into)] suggested_tags: Signal<Vec<String>>,

    #[prop(into)] on_click: Callback<TagOperation>
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
                children=move |(tag)| {
                    let tag_action = tag.1.clone();
                    view! {
                        <li on:click=move |_| {
                            on_click.call(tag_action.clone());
                        }>{match tag.1.clone() {

TagOperation::Untag(tag) => format!("remove tag {tag}"),
TagOperation::Tag(tag) => format!("add tag {tag}"),
                                               }                       }</li>
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

    let fetch_sticker_infos = create_action(|input: &(String, Option<TagOperation>)| {
        let input = input.clone();
        // async move { fetch_sticker_info(input.clone()).await }
        async move { tag_sticker(input.0, input.1).await }
    });

    let submitted = fetch_sticker_infos.input(); // RwSignal<Option<String>>
    let pending = fetch_sticker_infos.pending(); // ReadSignal<bool>
    let todo_id = fetch_sticker_infos.value(); // RwSignal<Option<Uuid>>

    let sticker_id_clone = sticker_id.clone();
    view! {
        <div on:mouseenter=move |ev| {
            if todo_id.get().is_none() {
                fetch_sticker_infos.dispatch((sticker_id.clone(), None));
            }
        }>
            <img
                src=format!("/files/stickers/{id}")
                alt=format!("sticker {id}")
                title=format!("sticker {id}")
                width="128"
                height="128"
                loading="lazy"
            />
            <p>{move || submitted.get().and_then(|_| Some("Submitted..."))}</p>
            <p>{move || pending.get().then(|| view! { <Loader/> })}</p>

            {move || match todo_id.get() {
                None => view! { <p>"hover to load..."</p> }.into_view(),
                Some(results) => {
                    match results {
                        Ok(info) => {
                            let sticker_id = sticker_id_clone.clone();
                            let fetch_sticker_infos = fetch_sticker_infos.clone();
                            view! {
                                // TODO: only fetch tags and suggested tags; no similarities
                                <Tagger
                                    on_click=move |tag_action: TagOperation| {
                                        fetch_sticker_infos.dispatch((sticker_id.clone(), Some(tag_action.clone())));
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

#[component]
pub fn Loader() -> impl IntoView {
    view! {
        <div class="flex items-center justify-center animate-pulse">
            <img
                class="motion-safe:animate-bounce m-4"
                src="/assets/fuzzle.svg"
                alt="loading animation"
                title="Loading ..."
                width="64"
                height="64"
            />
        </div>
    }
}
