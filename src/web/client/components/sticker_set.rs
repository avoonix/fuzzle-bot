use leptos::html::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use super::StickerSetDto;

#[component]
pub fn StickerSet(#[prop(into)] sticker_set: StickerSetDto) -> impl IntoView {
    view! {
        <span class="">
            {sticker_set.title.map_or_else(|| sticker_set.id.clone(), | title| 
                format!("{title} ({})", sticker_set.id)
            )}
        </span>
    }
}
