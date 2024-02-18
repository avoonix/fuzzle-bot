use std::vec;

use itertools::Itertools;
use leptos::html::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};

use crate::web::shared::*;

#[component]
pub fn Sticker(id: String) -> impl IntoView {
    view! {
        <div>
            <img
                src=format!("/files/stickers/{id}")
                alt=format!("sticker {id}")
                title=format!("sticker {id}")
                width="128"
                height="128"
                loading="lazy"
            />
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
