mod telegram_external;
mod sticker_service;
mod import_service;
mod similarity_service;
mod inference_service;

use std::sync::Arc;

pub use telegram_external::*;
pub use sticker_service::*;
pub use import_service::*;
pub use similarity_service::*;
pub use inference_service::*;

use crate::{Config, bot::Bot, database::Database, qdrant::VectorDatabase};

#[derive(Clone)]
pub struct Services {
    pub telegram: ExternalTelegramService,
    pub sticker: StickerService,
    pub import: ImportService,
    pub similarity: SimilarityService,
}

impl Services {
    pub fn new(config: Arc<Config>, database: Database, vector_db: VectorDatabase, bot: Bot) -> Self {
        let telegram = ExternalTelegramService::new(&config.external_telegram_service_base_url);
        let import = ImportService::new(database.clone(), config.clone(), bot, vector_db.clone(), telegram.clone());

        Self {
            // ban: BanService::new(database.clone(), import.clone(), vector_db.clone()),
            sticker: StickerService::new(database.clone()),
            similarity: SimilarityService::new(database, vector_db, import.clone()),
            import,
            telegram,
        }
    }
}
