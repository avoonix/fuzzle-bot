use itertools::Itertools;
use leptos::*;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr, vec};

use crate::{callback::TagOperation, sticker::Measures};

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
                0,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagDto {
    pub tags: Vec<String>,
    pub suggested_tags: Vec<String>, // TODO: use tagdto for stickerinfodto
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
        data.tagging_worker.clone(),
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

    let embedding_cosine = compute_similar(
        data.database.clone(),
        id.clone(),
        crate::inline::SimilarityAspect::Embedding,
        data.analysis_worker.clone(),
        50,
    )
    .await?;
    let histogram_cosine = compute_similar(
        data.database.clone(),
        id.clone(),
        crate::inline::SimilarityAspect::Color,
        data.analysis_worker.clone(),
        50,
    )
    .await?;
    let visual_hash_cosine = compute_similar(
        data.database.clone(),
        id.clone(),
        crate::inline::SimilarityAspect::Shape,
        data.analysis_worker.clone(),
        50,
    )
    .await?;

    // TODO: this could take a while
    // let similar = compute_similar(data.database.clone(), id.clone())
    //     .await
    //     .map_err(|err| {
    //         dbg!(err);
    //         ErrorInternalServerError("could not compute similar")
    //     })?;

    let similar = Measures {
        embedding_cosine,
        histogram_cosine,
        visual_hash_cosine,
    };

    Ok(StickerInfoDto {
        id,
        tags,
        suggested_tags,
        file_hash: sticker.file_hash,
        other_sets,
        similar,
    })
}

/// no operation = get tags, otherwise perform the selected operation and then get the new tags
#[server]
pub async fn tag_sticker(
    sticker_id: String,
    operation: Option<TagOperation>,
) -> Result<TagDto, ServerFnError> {
    use self::ssr::*;
    let (user, data): (AuthenticatedUser, Data<AppState>) = extract().await?;

    if !user.user_meta.can_tag_stickers() {
        return Err(ErrorInternalServerError("not allowed"))?;
    }

    // let sticker = data
    //     .database
    //     .get_sticker(id.clone())
    //     .await?
    //     .ok_or(ErrorNotFound("sticker not found"))?;
    // let tags = data.database.get_sticker_tags(id.clone()).await?;
    // let suggested_tags = suggest_tags(
    //     &id,
    //     data.bot.clone(),
    //     data.tag_manager.clone(),
    //     data.database.clone(),
    // )
    // .await
    // .map_err(|err| ErrorInternalServerError("tag suggestion error"))?;

    // let sets = data
    //     .database
    //     .get_sets_containing_sticker(id.clone())
    //     .await?;

    // let other_sets = sets
    //     .into_iter()
    //     .map(|s| StickerSetDto {
    //         id: s.id,
    //         title: s.title,
    //     })
    //     .collect_vec();

    // // TODO: this could take a while
    // let similar = compute_similar(data.database.clone(), id.clone())
    //     .await
    //     .map_err(|err| {
    //         dbg!(err);
    //         ErrorInternalServerError("could not compute similar")
    //     })?;

    match operation {
        None => {}
        Some(TagOperation::Tag(tag)) => {
            if let Some(implications) = data.tag_manager.get_implications(&tag) {
                let tags = implications
                    .clone()
                    .into_iter()
                    .chain(std::iter::once(tag.clone()))
                    .collect_vec();
                data.database
                    .tag_sticker(sticker_id.clone(), tags, Some(user.user_meta.id().0))
                    .await?;
                data.tagging_worker.maybe_recompute().await?;
            } else {
                dbg!("invalid tag");
            }
        }
        Some(TagOperation::Untag(tag)) => {
            data.database
                .untag_sticker(sticker_id.clone(), tag.clone(), user.user_meta.id().0)
                .await?;
            let implications = data.tag_manager.get_implications(&tag);
            // if let Some(implications) = implications {
            //     // if implications.is_empty() {
            //     //     "Removed!".to_string()
            //     // } else {
            //     //     format!(
            //     //         "Removed just this tag! ({tag} implies {})",
            //     //         implications.join(", ")
            //     //     )
            //     // }
            // }
        }
    }

    let tags = data.database.get_sticker_tags(sticker_id.clone()).await?;
    let suggested_tags = suggest_tags(
        &sticker_id,
        data.bot.clone(),
        data.tag_manager.clone(),
        data.database.clone(),
        data.tagging_worker.clone(),
    )
    .await
    .map_err(|_| ErrorInternalServerError("database error"))?;

    Ok(TagDto {
        tags,
        suggested_tags,
    })
}
