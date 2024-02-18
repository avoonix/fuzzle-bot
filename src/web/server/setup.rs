use std::{pin::Pin, sync::Arc};

use actix_files::Files;
use actix_web::{
    cookie::{
        time::{Duration, OffsetDateTime},
        Cookie, SameSite,
    },
    error::ErrorBadRequest,
    get,
    http::header,
    middleware, post, web, App, FromRequest, HttpRequest, HttpResponse, HttpServer, Responder,
};
use chrono::naive::serde::ts_seconds;
use futures::{
    future::{err, ok, Ready},
    Future,
};
use itertools::Itertools;
use leptos::*;
use leptos_actix::{generate_route_list, LeptosRoutes};
use ring::{digest, hmac};
use serde::{Deserialize, Serialize};
use teloxide::requests::Requester;

use crate::{
    bot::{get_or_create_user, Bot, UserMeta},
    database::Database,
    sticker::fetch_sticker_file,
    tags::TagManager,
    worker::WorkerPool,
    Config, Paths,
};

use super::service;

#[derive(Debug)]
pub struct AppState {
    pub config: Config,
    pub database: Database,
    pub tag_manager: Arc<TagManager>,
    pub worker: WorkerPool,
    pub bot: Bot,
    pub paths: Paths,
}

pub fn setup(
    config: Config,
    database: Database,
    tag_manager: Arc<TagManager>,
    worker: WorkerPool,
    bot: Bot,
    paths: Paths,
) {
    tokio::spawn(async move {
        let conf = get_configuration(None).await.expect("config to exist");
        let addr = conf.leptos_options.site_addr;
        let routes = generate_route_list(crate::web::client::App);
        println!("listening on http://{}", &addr);
        HttpServer::new(move || {
            let leptos_options = &conf.leptos_options;
            let site_root = &leptos_options.site_root;
            App::new()
                .app_data(web::Data::new(AppState {
                    config: config.clone(),
                    database: database.clone(),
                    tag_manager: tag_manager.clone(),
                    worker: worker.clone(),
                    bot: bot.clone(),
                    paths: paths.clone(),
                }))
                .service(Files::new("/pkg", format!("{site_root}/pkg")))
                .service(Files::new("/assets", site_root))
                .service(service::favicon)
                .service(service::login)
                .service(service::logout)
                .service(service::sticker_files)
                .service(service::histogram_files)
                .leptos_routes(
                    leptos_options.to_owned(),
                    routes.to_owned(),
                    crate::web::client::App,
                )
                .app_data(web::Data::new(leptos_options.to_owned()))
                .wrap(middleware::Compress::default())
        })
        .bind(&addr)
        .expect("address not to be in use")
        .run()
        .await
        .expect("server not to die")
    });
}
