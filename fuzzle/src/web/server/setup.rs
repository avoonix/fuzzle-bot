use std::sync::Arc;

use actix_files::Files;
use actix_web::{web::route, middleware, web, App, HttpServer};

use crate::{
    background_tasks::{TagManagerService, TfIdfService}, bot::Bot, database::Database, qdrant::VectorDatabase, web::server::page, Config
};

use super::service;

pub struct AppState {
    pub config: Arc<Config>,
    pub database: Database,
    pub tag_manager: TagManagerService,
    pub bot: Bot,
    pub tfidf_service: TfIdfService,
    // pub tag_worker: TagWorker,
    pub vector_db: VectorDatabase,
}

pub fn setup(
    config: Arc<Config>,
    database: Database,
    tag_manager: TagManagerService,
    bot: Bot,
    tfidf_service: TfIdfService,
    // tag_worker: TagWorker,
    vector_db: VectorDatabase,
) {
    tokio::spawn(async move {
        let addr = config.http_listen_address.clone();
        tracing::info!("listening on http://{}", addr);
        HttpServer::new(move || {
            App::new()
                .wrap(tracing_actix_web::TracingLogger::default())
                .app_data(web::Data::new(AppState {
                    config: config.clone(),
                    database: database.clone(),
                    tag_manager: tag_manager.clone(),
                    bot: bot.clone(),
                    tfidf_service: tfidf_service.clone(),
                    // tag_worker: tag_worker.clone(),
                    vector_db: vector_db.clone(),
                }))
                // .service(Files::new("/pkg", format!("{site_root}/pkg")))
                // .service(Files::new("/assets", site_root))
                // .service(service::favicon)
                .service(service::login)
                .service(service::login_webapp)
                .service(service::logout)
                .service(service::favicon)
                .service(service::asset_folder)
                .service(service::sticker_files)
                // .service(service::merge_files)
                .service(service::sticker_set_thumbnail)
                .service(service::sticker_comparison_thumbnail)
                .service(page::index)
                .service(page::search_tags)
                .service(page::sticker_set)
                .service(page::sticker_page)
                .service(page::tag_page)
                .service(page::emoji_page)
                .service(page::webapp_entrypoint)
                .service(page::sticker_set_timeline_page)
                // TODO: assets should be the default route
                .default_service(route().to(page::not_found))
                .wrap(middleware::Compress::default())
        })
        .bind(addr)
        .expect("address not to be in use")
        .run()
        .await
        .expect("server not to die");
    });
}
