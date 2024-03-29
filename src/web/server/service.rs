


use actix_web::{
    cookie::{
        time::{Duration},
        Cookie, SameSite,
    },
    error::{ErrorInternalServerError, ErrorNotFound, ErrorUnauthorized},
    get,
    http::header, web, HttpRequest, HttpResponse, Responder,
};
use actix_web_lab::extract::{Path, Query};



use leptos::LeptosOptions;





use crate::sticker::{create_historgram_image, fetch_possibly_cached_sticker_file, generate_merge_image};
use web::Data;

use crate::web::server::auth::AUTH_COOKIE_NAME;
use crate::web::server::{AppState, AuthData, AuthenticatedUser};

#[actix_web::get("favicon.ico")]
async fn favicon(leptos_options: Data<LeptosOptions>) -> actix_web::Result<actix_files::NamedFile> {
    let leptos_options = leptos_options.into_inner();
    let site_root = &leptos_options.site_root;
    Ok(actix_files::NamedFile::open(format!(
        "{site_root}/favicon.ico"
    ))?)
}

#[actix_web::get("/files/stickers/{sticker_id}")]
async fn sticker_files(
    Path(sticker_id): Path<String>,
    data: Data<AppState>,
    user: AuthenticatedUser,
) -> actix_web::Result<impl Responder> {
    let analysis = data
        .database
        .get_analysis_for_sticker_id(sticker_id)
        .await
        .map_err(|err| ErrorInternalServerError("database error"))? // TODO: better error handling
        .ok_or(ErrorNotFound("not found"))?;
    let file_id = analysis
        .thumbnail_file_id
        .ok_or(ErrorNotFound("no file id found"))?;
    let buf =
        fetch_possibly_cached_sticker_file(file_id, data.bot.clone(), data.paths.image_cache())
            .await
            .map_err(|err| ErrorInternalServerError("fetch error"))?;
    Ok(HttpResponse::Ok().body(buf))
}

#[actix_web::get("/files/histograms/{sticker_id}")]
async fn histogram_files(
    Path(sticker_id): Path<String>,
    data: Data<AppState>,
    user: AuthenticatedUser,
) -> actix_web::Result<impl Responder> {
    let analysis = data
        .database
        .get_analysis_for_sticker_id(sticker_id)
        .await
        .map_err(|err| ErrorInternalServerError("database error"))? // TODO: better error handling
        .ok_or(ErrorNotFound("not found"))?;
    let histogram = analysis.histogram.ok_or(ErrorNotFound("histogram not calculated"))?;
    let buf = create_historgram_image(histogram.into())
            .map_err(|err| ErrorInternalServerError("histogram encode error"))?;
    Ok(HttpResponse::Ok().body(buf))
}

#[actix_web::get("/files/merge/{sticker_id_a}/{sticker_id_b}")]
async fn merge_files(
    Path((sticker_id_a, sticker_id_b)): Path<(String, String)>,
    data: Data<AppState>,
    user: AuthenticatedUser,
) -> actix_web::Result<impl Responder> {
    let buf = generate_merge_image(sticker_id_a, sticker_id_b, data.database.clone(), data.bot.clone()).await
            .map_err(|err| {dbg!(err); ErrorInternalServerError("merge image error") })?;
    Ok(HttpResponse::Ok().body(buf))
}

#[get("logout")]
async fn logout(user: AuthenticatedUser) -> impl Responder {
    let mut cookie = Cookie::new(AUTH_COOKIE_NAME, "");
    cookie.make_removal();

    HttpResponse::Ok().cookie(cookie).body("logged out")
}

#[get("login")]
async fn login(
    Query(info): Query<AuthData>,
    data: Data<AppState>,
    request: HttpRequest,
) -> actix_web::Result<impl Responder> {
    if !info.check(data.config.telegram.token.clone()) {
        return Err(ErrorUnauthorized("invalid auth data"));
    }
    let cookie = Cookie::build(AUTH_COOKIE_NAME, serde_json::to_string(&info)?)
        .max_age(Duration::DAY * 5)
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
