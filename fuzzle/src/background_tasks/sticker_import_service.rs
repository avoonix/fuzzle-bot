use std::sync::Arc;

use flume::Sender;
use itertools::Itertools;
use teloxide::types::UserId;

use crate::{
    Config, bot::{Bot, InternalError, report_periodic_task_error}, database::Database, qdrant::VectorDatabase, services::ExternalTelegramService, sticker::import_all_stickers_from_set
};

#[derive(Clone)]
pub struct StickerImportService {
    database: Database,
    config: Arc<Config>,
    bot: Bot,
    vector_db: VectorDatabase,
    tx: Sender<StickerSetFetchRequest>,
    tg_service: ExternalTelegramService,
}

struct StickerSetFetchRequest {
    set_id: String,
    ignore_last_fetched: bool,
    user_id: Option<UserId>,
}

impl StickerImportService {
    #[tracing::instrument(skip(database, config, tg_service, bot, vector_db))]
    pub async fn new(
        database: Database,
        config: Arc<Config>,
        bot: Bot,
        vector_db: VectorDatabase,
        tg_service: ExternalTelegramService,
    ) -> Result<Self, InternalError> {
        let (tx, rx) = flume::unbounded();
        let service = Self {
            database,
            config,
            bot,
            vector_db,
            tx,
            tg_service,
        };
        {
            let service = service.clone();
            tokio::spawn(async move {
                while let Ok(received) = rx.recv_async().await {
                    let StickerSetFetchRequest {
                        set_id,
                        ignore_last_fetched,
                        user_id,
                    } = received;
                    let result = import_all_stickers_from_set(
                        &set_id,
                        ignore_last_fetched,
                        service.bot.clone(),
                        service.database.clone(),
                        service.config.clone(),
                        service.vector_db.clone(),
                        user_id,
                        service.tg_service.clone(),
                    )
                    .await;
                    report_periodic_task_error(result);
                }
            });
        }
        Ok(service)
    }

    pub async fn queue_sticker_set_import(
        &self,
        set_id: &str,
        ignore_last_fetched: bool,
        user_id: Option<UserId>,
    ) {
        self.tx
            .send_async(StickerSetFetchRequest {
                set_id: set_id.to_string(),
                ignore_last_fetched,
                user_id,
            })
            .await
            .expect("channel must be open");
    }

    /// avoid adding already imported sticker sets to the queue if busy
    pub fn is_busy(&self) -> bool {
        self.tx.len() > 100
    }
}
