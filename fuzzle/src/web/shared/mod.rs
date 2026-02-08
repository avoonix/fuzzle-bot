pub mod errors;
use std::sync::Arc;
use actix_web_lab::{
    extract::{Path, Query},
    header::{CacheControl, CacheDirective},
};

use crate::{
    Config, background_tasks::{ TagManagerService, TfIdfService}, bot::Bot, database::Database, qdrant::VectorDatabase, services::Services, web::server::page
};

pub struct AppState {
    pub config: Arc<Config>,
    pub database: Database,
    pub tag_manager: TagManagerService,
    pub bot: Bot,
    pub tfidf_service: TfIdfService,
    // pub tag_worker: TagWorker,
    pub vector_db: VectorDatabase,
    pub services: Services,
}


pub const MINUTE: u32 = 60;
pub const HOUR: u32 = MINUTE * 60;
pub const DAY: u32 = HOUR * 24;

pub fn thumbnail_cache_control_header() -> CacheControl {
    CacheControl(if cfg!(debug_assertions) {
        vec![
            CacheDirective::Public,
            CacheDirective::StaleWhileRevalidate,
            CacheDirective::StaleIfError,
            CacheDirective::MaxAge(7 * DAY),
        ]
    } else { vec![
        CacheDirective::Public,
        CacheDirective::StaleWhileRevalidate,
        CacheDirective::StaleIfError,
        CacheDirective::MaxAge(30 * DAY),
    ]})
}
