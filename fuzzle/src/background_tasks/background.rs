use tracing::{warn, Instrument};

use crate::{
    bot::{RequestContext},
    sticker::{analyze_sticker, import_all_stickers_from_set},
};
use crate::bot::report_periodic_task_error;

pub trait BackgroundTaskExt {
    async fn process_sticker_set(&self, set_name: String, ignore_last_fetched: bool);
    async fn process_set_of_sticker(&self, sticker_unique_id: String);
    async fn analyze_sticker(&self, sticker_unique_id: String);
}

impl BackgroundTaskExt for RequestContext {
    async fn process_sticker_set(&self, set_name: String, ignore_last_fetched: bool) {
        // TODO: retry on error
        let bot = self.bot.clone();
        let admin_id = self.config.get_admin_user_id();
        let database = self.database.clone();
        let request_context = self.clone();
        let span = tracing::info_span!("spawned_process_sticker_set");
        tokio::spawn(async move {
            let result =
                import_all_stickers_from_set(&set_name, ignore_last_fetched, bot.clone(), database.clone(), request_context.config.clone(), request_context.vector_db.clone()).await;
            report_periodic_task_error(result);
        }.instrument(span));
    }

    async fn process_set_of_sticker(&self, sticker_unique_id: String) {
        let bot = self.bot.clone();
        let admin_id = self.config.get_admin_user_id();
        let database = self.database.clone();
        let request_context = self.clone();
        let span = tracing::info_span!("spawned_process_set_of_sticker");
        tokio::spawn(async move {
            let result = database.get_sticker_set_by_sticker_id(&sticker_unique_id).await;
            let set_id = match result {
                Ok(Some(sticker_set)) => sticker_set.id,
                Ok(None) => return warn!("sticker without set {sticker_unique_id}"),
                Err(err) => {
                    tracing::error!("error while getting sticker set name: {err:?}");
                    return;
                }
            };
            let result =
                import_all_stickers_from_set(&set_id, false, bot.clone(), database.clone(), request_context.config.clone(), request_context.vector_db.clone()).await;
            report_periodic_task_error(result);
        }.instrument(span));
    }

    #[tracing::instrument(skip(self))]
    async fn analyze_sticker(&self, sticker_unique_id: String) {
        let bot = self.bot.clone();
        let admin_id = self.config.get_admin_user_id();
        let database = self.database.clone();
        let config = self.config.clone();
        let source_user = self.user_id();
        let request_context = self.clone();
        let vector_db = self.vector_db.clone();

        tokio::spawn(async move {
            let result = database.get_sticker_file_by_sticker_id(&sticker_unique_id).await;
            let file = match result {
                Ok(None) => return,
                Ok(Some(analysis)) => analysis,
                Err(err) => {
                    tracing::error!("database error while getting file info: {err:?}");
                    return;
                }
            };
            let result = vector_db.find_missing_stickers(vec![file.id]).await;
            let missing = match result {
                Ok(missing) => missing,
                Err(err) => {
                    tracing::error!("vector database error while getting missing files: {err}");
                    return;
                }
            };
            if !missing.is_empty() {
                let result = analyze_sticker(sticker_unique_id, database, bot.clone(), config, vector_db).await;
                report_periodic_task_error(result);
            }
        }.instrument(tracing::info_span!(parent: tracing::Span::none(), "analyze_sticker_background_task")));
    }
}
