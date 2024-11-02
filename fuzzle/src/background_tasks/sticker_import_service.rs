use std::sync::Arc;

use flume::Sender;
use itertools::Itertools;
use teloxide::types::UserId;

use crate::{
    bot::{report_periodic_task_error, Bot, InternalError},
    database::Database,
    qdrant::VectorDatabase,
    sticker::import_all_stickers_from_set,
    Config,
};

#[derive(Clone)]
pub struct StickerImportService {
    database: Database,
    config: Arc<Config>,
    bot: Bot,
    vector_db: VectorDatabase,
    tx: Sender<StickerSetFetchRequest>,
}

struct StickerSetFetchRequest {
    set_id: String,
    ignore_last_fetched: bool,
    user_id: Option<UserId>,
}

impl StickerImportService {
    #[tracing::instrument(skip(database, config))]
    pub async fn new(
        database: Database,
        config: Arc<Config>,
        bot: Bot,
        vector_db: VectorDatabase,
    ) -> Result<Self, InternalError> {
        let (tx, rx) = flume::unbounded();
        let service = Self {
            database,
            config,
            bot,
            vector_db,
            tx,
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
