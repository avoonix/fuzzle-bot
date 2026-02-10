use std::collections::HashSet;

use actix_web::{
    HttpRequest, HttpResponse, Responder,
    cookie::{Cookie, SameSite, time::Duration},
    error::{ErrorInternalServerError, ErrorNotFound},
    get,
    http::header,
    web::{self, Json},
};
use actix_web_lab::{
    extract::{Path, Query},
    header::{CacheControl, CacheDirective},
};
use itertools::Itertools;
use serde::Deserialize;

use crate::{
    bot::{BotError, InternalError},
    database::{BanReason, BannedSticker, Sticker, StickerSet},
    services::Services,
    sticker::{
        create_historgram_image, create_sticker_thumbnail, fetch_sticker_file, generate_merge_image,
    },
    util::Required,
    web::shared::{AppState, thumbnail_cache_control_header},
};
use web::Data;

#[derive(serde::Serialize)]
struct StickerSetPub {
    id: String,
    title: Option<String>,
}

impl From<StickerSet> for StickerSetPub {
    fn from(value: StickerSet) -> Self {
        Self {
            id: value.id,
            title: value.title,
        }
    }
}

#[derive(serde::Serialize)]
struct StickerPub {
    id: String,
    set_id: String,
}

#[derive(serde::Serialize)]
struct StickerWithSimilarityPub {
    #[serde(flatten)]
    sticker: StickerPub,
    similarity: f32,
}

impl From<Sticker> for StickerPub {
    fn from(value: Sticker) -> Self {
        Self {
            id: value.id,
            set_id: value.sticker_set_id,
        }
    }
}

impl From<BannedSticker> for StickerPub {
    fn from(value: BannedSticker) -> Self {
        Self {
            id: value.id,
            set_id: value.sticker_set_id,
        }
    }
}

#[actix_web::get("/api/pending-sets")]
#[tracing::instrument(skip(data))]
async fn get_pending_sets(data: Data<AppState>) -> actix_web::Result<impl Responder> {
    let set = data.database.get_pending_sticker_sets(100, 0).await?;
    Ok(actix_web::web::Json(
        set.into_iter()
            .map(|s| StickerSetPub::from(s))
            .collect_vec(),
    ))
}

#[actix_web::post("/api/sets/{set_id}/ban")]
#[tracing::instrument(skip(data))]
async fn ban_set(
    Path(set_id): Path<String>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    data.services.import.ban_sticker_set(&set_id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[actix_web::post("/api/sets/{set_id}/unban")]
#[tracing::instrument(skip(data))]
async fn unban_set(
    Path(set_id): Path<String>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    data.services.import.unban_sticker_set(&set_id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
struct StickerBanBody {
    clip_max_match_distance: f32,
}

#[actix_web::get("/api/stickers/{sticker_id}/similar")]
#[tracing::instrument(skip(data))]
async fn get_similar_stickers(
    Path(sticker_id): Path<String>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    let (matches, _) = data
        .services
        .similarity
        .find_similar_stickers(
            sticker_id,
            crate::inline::SimilarityAspect::Embedding,
            100,
            0,
        )
        .await?;
    let result = data
        .services
        .similarity
        .matches_to_stickers(matches)
        .await?;
    Ok(actix_web::web::Json(
        result
            .into_iter()
            .map(|(s, similarity)| StickerWithSimilarityPub {
                sticker: StickerPub::from(s),
                similarity,
            })
            .collect_vec(),
    ))
}

#[actix_web::post("/api/stickers/{sticker_id}/ban")]
#[tracing::instrument(skip(data, body))]
async fn ban_sticker(
    Path(sticker_id): Path<String>,
    body: Json<StickerBanBody>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    data.services
        .import
        .ban_sticker(&sticker_id, body.clip_max_match_distance, BanReason::Manual)
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[actix_web::post("/api/stickers/{sticker_id}/unban")]
#[tracing::instrument(skip(data))]
async fn unban_sticker(
    Path(sticker_id): Path<String>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    data.services.import.unban_sticker(&sticker_id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[actix_web::get("/api/sets/{set_id}/stickers")]
#[tracing::instrument(skip(data))]
async fn get_stickers_in_set(
    Path(set_id): Path<String>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    let sticker_set = data.database.get_sticker_set_by_id(&set_id).await?;
    if sticker_set.is_none() {
        return Err(InternalError::UnexpectedNone { type_name: "sticker set".to_string() }.into()) // TODO: do not use that error
    }
    let stickers = data.database.get_all_stickers_in_set(&set_id).await?;
    Ok(actix_web::web::Json(
        stickers
            .into_iter()
            .map(|s| StickerPub::from(s))
            .collect_vec(),
    ))
}

#[actix_web::post("/api/sets/{set_id}/approve")]
#[tracing::instrument(skip(data))]
async fn approve_set(
    Path(set_id): Path<String>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    data.database
        .set_sticker_set_pending(&set_id, false)
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[actix_web::post("/api/scan-all-stickers-for-bans")]
#[tracing::instrument(skip(data))]
async fn scan_all_stickers_for_bans(data: Data<AppState>) -> actix_web::Result<impl Responder> {
    tokio::task::spawn(async move {
        let result: Result<(), InternalError> = async {
            let stickers = data.vector_db.scroll_stickers().await?;
            dbg!(stickers.len());
            dbg!(stickers.first());
            for (clip_vec, histogram_vec, file_id) in stickers {
                let sticker = data.database.get_some_sticker_by_file_id(&file_id).await?;
                if let Some(sticker) = sticker {
                    data.services
                        .import
                        .possibly_auto_ban_sticker(clip_vec, &sticker.id, &sticker.sticker_set_id)
                        .await?;
                }
            }
            Ok(())
        }
        .await;
        match result {
            Ok(_) => tracing::info!("finished scan"),
            Err(err) => tracing::error!(error = %err, "periodic task error: {err:?}"),
        }
    });
    Ok(HttpResponse::Ok().finish())
}

#[actix_web::get("/api/banned-stickers")]
#[tracing::instrument(skip(data))]
async fn get_banned_stickers(data: Data<AppState>) -> actix_web::Result<impl Responder> {
    let stickers = data.database.get_banned_stickers(100, 0).await?;
    Ok(actix_web::web::Json(
        stickers
            .into_iter()
            .map(|s| StickerPub::from(s))
            .collect_vec(),
    ))
}

#[actix_web::get("/api/banned-sticker/{sticker_id}/thumbnail.png")]
#[tracing::instrument(skip(data))]
async fn get_banned_sticker_thumbnail(
    Path(sticker_id): Path<String>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    // TODO: deduplicate from other public service
    let sticker = data
        .database
        .get_banned_sticker(&sticker_id)
        .await?
        .required()?;
    let file_id = sticker.thumbnail_file_id.required()?;
    let (buf, _) = fetch_sticker_file(file_id, data.bot.clone()).await?;
    Ok(HttpResponse::Ok()
        .insert_header(thumbnail_cache_control_header())
        .insert_header(header::ContentType::png())
        .body(buf))
}
