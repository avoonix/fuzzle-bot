use std::vec;

use itertools::Itertools;
use leptos::html::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};

use crate::web::shared::*;

#[component]
pub fn Sticker(data: StickerDto) -> impl IntoView {
    view! {
        <div>
            <img
                src=format!("/files/stickers/{}", data.id)
                alt=format!("sticker {}", data.id)
                width="128"
                height="128"
                loading="lazy"
            />
        </div>
    }
}
