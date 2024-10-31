use tracing::{warn, Instrument};

use crate::bot::report_periodic_task_error;
use crate::{bot::RequestContext, sticker::analyze_sticker};

pub trait BackgroundTaskExt {
    async fn analyze_sticker(&self, sticker_unique_id: String);
}

impl BackgroundTaskExt for RequestContext {
    // TODO: should we also add a queue for the analyze task?
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
