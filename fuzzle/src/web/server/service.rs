use std::collections::HashSet;

use actix_web::{
    cookie::{time::Duration, Cookie, SameSite},
    error::{ErrorInternalServerError, ErrorNotFound, ErrorUnauthorized},
    get,
    http::header,
    web, HttpRequest, HttpResponse, Responder,
};
use actix_web_lab::{
    extract::{Path, Query},
    header::{CacheControl, CacheDirective},
};
use itertools::Itertools;

use crate::{sticker::{
    create_historgram_image, create_sticker_thumbnail, fetch_sticker_file, generate_merge_image
}, util::Required};
use web::Data;

use crate::web::server::auth::AUTH_COOKIE_NAME;
use crate::web::server::{AppState, AuthData, AuthenticatedUser};

// TODO: serve static files like robots.txt and favicon.ico

#[actix_web::get("/files/stickers/{sticker_id}")]
#[tracing::instrument(skip(data, user))]
async fn sticker_files(
    Path(sticker_id): Path<String>,
    data: Data<AppState>,
    user: AuthenticatedUser,
) -> actix_web::Result<impl Responder> {
    let file = data
        .database
        .get_sticker_file_by_sticker_id(&sticker_id)
        .await?
        .required()?;
    let file_id = file
        .thumbnail_file_id
        .required()?;
    let (buf, _) = fetch_sticker_file(file_id, data.bot.clone()) .await?;
    Ok(HttpResponse::Ok().body(buf))
}

#[actix_web::get("/files/merge/{sticker_id_a}/{sticker_id_b}")]
#[tracing::instrument(skip(data, user))]
async fn merge_files(
    Path((sticker_id_a, sticker_id_b)): Path<(String, String)>,
    data: Data<AppState>,
    user: AuthenticatedUser,
) -> actix_web::Result<impl Responder> {
    let buf = generate_merge_image(
        &sticker_id_a,
        &sticker_id_b,
        data.database.clone(),
        data.bot.clone(),
    )
    .await?;
    Ok(HttpResponse::Ok().body(buf))
}

#[actix_web::get("/thumbnails/sticker-set/{setId}")]
#[tracing::instrument(skip(data))]
async fn sticker_set_thumbnail(
    Path(set_id): Path<String>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    let stickers = data.database.get_all_stickers_in_set(&set_id).await?;
    let files = data
        .database
        .get_sticker_files_by_ids(
            stickers
                .into_iter()
                .map(|s| s.sticker_file_id)
                .collect_vec()
                .as_ref(),
        )
        .await?;
    let buf = create_sticker_thumbnail(files, 400, data.bot.clone(), data.config.clone())
        .await?;
    Ok(HttpResponse::Ok()
        .insert_header(thumbnail_cache_control_header())
        .body(buf))
}

#[actix_web::get("/thumbnails/compare-sticker-sets/{setId1}/{setId2}")]
#[tracing::instrument(skip(data))]
async fn sticker_comparison_thumbnail(
    Path((set_id_a, set_id_b)): Path<(String, String)>,
    data: Data<AppState>,
) -> actix_web::Result<impl Responder> {
    let stickers_a = data.database.get_all_stickers_in_set(&set_id_a).await?;
    let stickers_b = data.database.get_all_stickers_in_set(&set_id_b).await?;
    let file_hashes: HashSet<_> = stickers_a.into_iter().map(|s| s.sticker_file_id).collect();
    let common = stickers_b
        .into_iter()
        .map(|s| s.sticker_file_id)
        .filter(|s| file_hashes.contains(s))
        .collect_vec();
    let files = data.database.get_sticker_files_by_ids(&common).await?;
    let buf = create_sticker_thumbnail(files, 400, data.bot.clone(), data.config.clone())
        .await?;
    Ok(HttpResponse::Ok()
        .insert_header(thumbnail_cache_control_header())
        .body(buf))
}

const MINUTE: u32 = 60;
const HOUR: u32 = MINUTE * 60;
const DAY: u32 = HOUR * 24;

fn thumbnail_cache_control_header() -> CacheControl {
    CacheControl(vec![
        CacheDirective::Public,
        CacheDirective::StaleWhileRevalidate,
        CacheDirective::StaleIfError,
        CacheDirective::MaxAge(10 * HOUR),
    ])
}

#[get("logout")]
#[tracing::instrument]
async fn logout(user: AuthenticatedUser) -> impl Responder {
    let mut cookie = Cookie::new(AUTH_COOKIE_NAME, "");
    cookie.make_removal();

    HttpResponse::Ok().cookie(cookie).body("logged out")
}

#[get("login")]
#[tracing::instrument(skip(data, request))]
async fn login(
    Query(info): Query<AuthData>,
    data: Data<AppState>,
    request: HttpRequest,
) -> actix_web::Result<impl Responder> {
    if !info.check(data.config.telegram_bot_token.clone()) {
        return Err(ErrorUnauthorized("invalid auth data"));
    }
    let cookie = Cookie::build(AUTH_COOKIE_NAME, serde_json::to_string(&info)?)
        .max_age(Duration::DAY * 30)
        .same_site(SameSite::Lax)
        .secure(true)
        .http_only(true)
        .finish();
    Ok(HttpResponse::Ok()
        .cookie(cookie)
        .insert_header((header::LOCATION, "/"))
        .status(actix_web::http::StatusCode::TEMPORARY_REDIRECT)
        .finish())
}
