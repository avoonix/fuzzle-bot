use itertools::Itertools;
use leptos::*;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr, vec};

use crate::sticker::Measures;

#[cfg(feature = "ssr")]
pub mod ssr {
    pub use crate::inline::query_stickers;
    pub use crate::inline::InlineQueryData;
    pub use crate::sticker::compute_similar;
    pub use crate::tags::suggest_tags;
    pub use crate::web::server::AppState;
    pub use crate::web::server::AuthenticatedUser;
    pub use actix_web::error::ErrorInternalServerError;
    pub use actix_web::error::ErrorNotFound;
    pub use actix_web::web;
    pub use actix_web::web::{Data, Query};
    pub use actix_web::HttpRequest;
    pub use leptos::ServerFnError;
    pub use leptos_actix::extract;
}

#[server]
pub async fn fetch_results(
    query: String,
    limit: usize,
    offset: usize,
) -> Result<Vec<StickerDto>, ServerFnError> {
    use self::ssr::*;
    let (user, data): (AuthenticatedUser, Data<AppState>) = extract().await?;

    let query =
        InlineQueryData::try_from(query).map_err(|err| ServerFnError::new("invalid query"))?;

    let result = match query.mode.clone() {
        crate::inline::InlineQueryDataMode::StickerSearch { emoji } => {
            let result = query_stickers(
                query,
                data.database.clone(),
                emoji,
                user.user_meta,
                data.tag_manager.clone(),
                limit,
                offset,
                0
            )
            .await
            .map_err(|err| ServerFnError::new("bot error"))?; // TODO: limit, offset, proper error handling
            result
        }
        _ => Err(ServerFnError::new("query not implemented"))?,
    };

    Ok(result
        .into_iter()
        .map(|s| StickerDto { id: s.id })
        .collect_vec())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerDto {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerInfoDto {
    pub id: String,
    pub tags: Vec<String>,
    pub suggested_tags: Vec<String>,
    pub file_hash: String,
    pub other_sets: Vec<StickerSetDto>,

    pub similar: Measures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerSetDto {
    pub id: String,
    pub title: Option<String>,
}

#[server]
pub async fn fetch_sticker_info(id: String) -> Result<StickerInfoDto, ServerFnError> {
    use self::ssr::*;
    let (user, data): (AuthenticatedUser, Data<AppState>) = extract().await?;

    let sticker = data
        .database
        .get_sticker(id.clone())
        .await?
        .ok_or(ErrorNotFound("sticker not found"))?;
    let tags = data.database.get_sticker_tags(id.clone()).await?;
    let suggested_tags = suggest_tags(
        &id,
        data.bot.clone(),
        data.tag_manager.clone(),
        data.database.clone(),
    )
    .await
    .map_err(|err| ErrorInternalServerError("tag suggestion error"))?;

    let sets = data
        .database
        .get_sets_containing_sticker(id.clone())
        .await?;

    let other_sets = sets
        .into_iter()
        .map(|s| StickerSetDto {
            id: s.id,
            title: s.title,
        })
        .collect_vec();

    // TODO: this could take a while
    let similar = compute_similar(data.database.clone(), id.clone())
        .await
        .map_err(|err| {
            dbg!(err);
            ErrorInternalServerError("could not compute similar")
        })?;

    Ok(StickerInfoDto {
        id,
        tags,
        suggested_tags,
        file_hash: sticker.file_hash,
        other_sets,
        similar,
    })
}
