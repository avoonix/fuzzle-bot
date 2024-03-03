

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
struct MergeParams {
    sticker_id_a: Option<String>,
    sticker_id_b: Option<String>,
}

#[component]
pub fn MergePage() -> impl IntoView {
    let params = use_params::<MergeParams>();
    let id_a =
        move || params.with(|params| params.as_ref().map(|params| 
            params.sticker_id_a.clone().unwrap_or_default()).unwrap_or_default());
    let id_b =
        move || params.with(|params| params.as_ref().map(|params| 
            params.sticker_id_b.clone().unwrap_or_default()).unwrap_or_default());

    let merge_info = create_resource(
        move || (id_a(), id_b()),
        |(a, b)| merge_infos(a, b),
    );

//     view! { <div>"sticker" {move || view! {
// <Sticker id=id()/>
//     }} 

//     <Loader />

//     </div> }
    view! {
        <div>
        <img
                src=move || format!("/files/merge/{}/{}", id_a(), id_b())
                alt="Sticker differences"
                loading="lazy"
                style="image-rendering: pixelated; width: 100%; height: 20vh; outline: 1px solid grey;"
            />

            <img
                src=format!("/files/histograms/{}", id_a())
                alt="Color histogram of sticker a"
                loading="lazy"
                style="image-rendering: pixelated; width: 100%; height: 20vh; outline: 1px solid grey;"
            />
            <img
                src=format!("/files/histograms/{}", id_b())
                alt="Color histogram of sticker b"
                loading="lazy"
                style="image-rendering: pixelated; width: 100%; height: 20vh; outline: 1px solid grey;"
            />

        <Transition fallback=move || ()>
            <div>
                {move || match merge_info.get() {
                    None => view! { <p>"Loading..."</p> }.into_view(),
                    Some(results) => {
                        match results {
                            Ok(info) => view! { <MergeInfo info /> }.into_view(),
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

            </div>
    }

}

#[component]
fn MergeInfo(#[prop(into)] info: StickerMergeInfos) -> impl IntoView {
    view! {
        <div class="">
    
        <div class="flex">
        <div>
        <ul class="">
            {info.sticker_a.sticker_sets.clone().into_iter() .map(|set| { view! { <li><StickerSet sticker_set=set /></li> } }) .collect_vec()}
        </ul>
        </div>
        <div>
        <ul class="">
            {info.sticker_b.sticker_sets.clone().into_iter() .map(|set| { view! { <li><StickerSet sticker_set=set /></li> } }) .collect_vec()}
        </ul>
        </div>
        </div>

        <code>{format!("{info:?}")}</code>
        </div>
    }
}

