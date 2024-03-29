use std::{sync::Arc};

use actix_files::Files;
use actix_web::{
    middleware, web, App, HttpServer,
};



use leptos::get_configuration;
use leptos_actix::{generate_route_list, LeptosRoutes};




use crate::{
    background_tasks::{AnalysisWorker, TaggingWorker}, bot::{Bot}, database::Database, tags::TagManager, Config, Paths
};

use super::service;

#[derive(Debug)]
pub struct AppState {
    pub config: Arc<Config>,
    pub database: Database,
    pub tag_manager: Arc<TagManager>,
    pub bot: Bot,
    pub paths: Arc<Paths>,
    pub analysis_worker: AnalysisWorker,
    pub tagging_worker: TaggingWorker,
}

pub fn setup(
    config: Arc<Config>,
    database: Database,
    tag_manager: Arc<TagManager>,
    bot: Bot,
    paths: Arc<Paths>,
    analysis_worker: AnalysisWorker,
    tagging_worker: TaggingWorker,
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
                    bot: bot.clone(),
                    paths: paths.clone(),
                    analysis_worker: analysis_worker.clone(),
                    tagging_worker: tagging_worker.clone(),
                }))
                .service(Files::new("/pkg", format!("{site_root}/pkg")))
                .service(Files::new("/assets", site_root))
                .service(service::favicon)
                .service(service::login)
                .service(service::logout)
                .service(service::sticker_files)
                .service(service::histogram_files)
                .service(service::merge_files)
                .leptos_routes(
                    leptos_options.to_owned(),
                    routes.clone(),
                    crate::web::client::App,
                )
                .app_data(web::Data::new(leptos_options.to_owned()))
                .wrap(middleware::Compress::default())
        })
        .bind(&addr)
        .expect("address not to be in use")
        .run()
        .await
        .expect("server not to die");
    });
}
