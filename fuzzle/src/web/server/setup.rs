use std::sync::Arc;

use actix_files::Files;
use actix_web::{middleware, web, App, HttpServer};

use crate::{
    background_tasks::TaggingWorker, bot::Bot, database::Database, qdrant::VectorDatabase, tags::TagManager, Config
};

use super::service;

pub struct AppState {
    pub config: Arc<Config>,
    pub database: Database,
    pub tag_manager: Arc<TagManager>,
    pub bot: Bot,
    pub tagging_worker: TaggingWorker,
    // pub tag_worker: TagWorker,
    pub vector_db: VectorDatabase,
}

pub fn setup(
    config: Arc<Config>,
    database: Database,
    tag_manager: Arc<TagManager>,
    bot: Bot,
    tagging_worker: TaggingWorker,
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
                    tagging_worker: tagging_worker.clone(),
                    // tag_worker: tag_worker.clone(),
                    vector_db: vector_db.clone(),
                }))
                // .service(Files::new("/pkg", format!("{site_root}/pkg")))
                // .service(Files::new("/assets", site_root))
                // .service(service::favicon)
                .service(service::login)
                .service(service::logout)
                .service(service::favicon)
                .service(service::asset_folder)
                // .service(service::sticker_files)
                // .service(service::merge_files)
                .service(service::sticker_set_thumbnail)
                .service(service::sticker_comparison_thumbnail)
                .wrap(middleware::Compress::default())
        })
        .bind(addr)
        .expect("address not to be in use")
        .run()
        .await
        .expect("server not to die");
    });
}
