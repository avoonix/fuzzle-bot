

use itertools::Itertools;
use leptos::html::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};

use crate::sticker::Match;
use crate::web::client::components::*;
use crate::web::shared::*;

#[derive(Params, PartialEq)]
struct StickerParams {
    sticker_id: Option<String>,
}

#[component]
pub fn StickerPage() -> impl IntoView {
    let params = use_params::<StickerParams>();
    let id =
        move || params.with(|params| params.as_ref().map(|params| params.sticker_id.clone().unwrap_or_default()).unwrap_or_default());

    let sticker_info = create_resource(
        id,
        fetch_sticker_info,
    );

    view! { <div>"sticker" {move || view! {
<Sticker id=id()/>
    }} 

    <Loader />

        <Transition fallback=move || ()>
            <div>
                {move || match sticker_info.get() {
                    None => view! { <p>"Loading..."</p> }.into_view(),
                    Some(results) => {
                        match results {
                            Ok(info) => view! { <StickerInfo info /> }.into_view(),
                            Err(err) => {
                                view! {
                                    <pre class="error">"Server Error: " {err.to_string()}</pre>
                                }
                                    .into_view()
                            }
                        }
                    }
                }}

            </div>
        </Transition>
    </div> }

}

#[component]
fn StickerInfo(#[prop(into)] info: StickerInfoDto) -> impl IntoView {
    view! {
        <div class="">
            <ul>
                <li>Id: {info.id.clone()} </li>
            </ul>
            
            <h2>Histogram</h2>

            <img
                src=format!("/files/histograms/{}", info.id)
                alt="Color histogram of sticker"
                loading="lazy"
                style="image-rendering: pixelated; width: 100%; height: 20vh; outline: 1px solid grey;"
            />

        // TODO: make page prettier
        <div class="flex"> histogram cosine
            {info.similar.histogram_cosine.clone().items() .into_iter() .map(|Match{distance, sticker_id}| { view! { <div style="width: 150px"> {format!("{distance}")} <A href=format!("/sticker/{sticker_id}")> <Sticker id=sticker_id.clone()/> </A> </div> } }) .collect_vec()}
        </div>
        <div class="flex"> visual hash cosine
            {info.similar.visual_hash_cosine.clone().items() .into_iter() .map(|Match{distance, sticker_id}| { view! { <div style="width: 150px"> {format!("{distance}")} <A href=format!("/sticker/{sticker_id}")> <Sticker id=sticker_id.clone()/> </A> </div> } }) .collect_vec()}
        </div>
        <div class="flex"> embedding cosine
            {info.similar.embedding_cosine.clone().items() .into_iter() .map(|Match{distance, sticker_id}| { view! { <div style="width: 150px"> {format!("{distance}")} <A href=format!("/sticker/{sticker_id}")> <Sticker id=sticker_id.clone()/> </A> </div> } }) .collect_vec()}
        </div>

        <code>{format!("{info:?}")}</code>
        </div>
    }
}

