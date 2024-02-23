use log::warn;


use crate::{
    bot::{log_error_and_send_to_admin, RequestContext},
    sticker::{analyze_n_stickers, import_all_stickers_from_set, notify_admin_if_set_new},
};

use super::{send_message_to_admin, AdminMessage};

pub trait BackgroundTaskExt {
    async fn send_message_to_admin_background(&self, msg: AdminMessage);
    async fn process_sticker_set(&self, set_name: String);
    async fn process_set_of_sticker(&self, sticker_unique_id: String);
}

impl BackgroundTaskExt for RequestContext {
    async fn send_message_to_admin_background(&self, msg: AdminMessage) {
        let bot = self.bot.clone();
        let admin_id = self.config.get_admin_user_id();
        let source_user = self.user_id();
        tokio::spawn(async move {
            let result = send_message_to_admin(msg, source_user, bot.clone(), admin_id).await;
            match result {
                Ok(()) => {}
                Err(err) => log_error_and_send_to_admin(err, bot.clone(), admin_id).await,
            };
        });
    }

    async fn process_sticker_set(&self, set_name: String) {
        // TODO: retry on error
        // TODO: add parameter ignore_last_fetched
        let bot = self.bot.clone();
        let admin_id = self.config.get_admin_user_id();
        let database = self.database.clone();
        let paths = self.paths.clone();
        let source_user = self.user_id();
        let worker = self.analysis_worker.clone();
        let request_context = self.clone();
        tokio::spawn(async move {
            let result = notify_admin_if_set_new(set_name.clone(), request_context.clone()).await;
            match result {
                Ok(()) => {}
                Err(err) => log_error_and_send_to_admin(err.into(), bot.clone(), admin_id).await,
            };
            let result = import_all_stickers_from_set(
                set_name,
                false,
                bot.clone(),
                database.clone(),
            )
            .await;
            match result {
                Ok(()) => {}
                Err(err) => log_error_and_send_to_admin(err, bot.clone(), admin_id).await,
            };
            let result = analyze_n_stickers(database, bot.clone(), 120, paths, worker.clone()).await;
            match result {
                Ok(()) => {}
                Err(err) => log_error_and_send_to_admin(err, bot.clone(), admin_id).await,
            };
        });
    }

    async fn process_set_of_sticker(&self, sticker_unique_id: String) {
        let bot = self.bot.clone();
        let admin_id = self.config.get_admin_user_id();
        let database = self.database.clone();
        tokio::spawn(async move {
            let result = database.get_set_name(sticker_unique_id.clone()).await;
            let set_name = match result {
                Ok(Some(set_name)) => set_name,
                Ok(None) => return warn!("sticker without set {sticker_unique_id}"),
                Err(err) => {
                    return log_error_and_send_to_admin(err.into(), bot.clone(), admin_id).await
                }
            };
            let result = import_all_stickers_from_set(
                set_name,
                false,
                bot.clone(),
                database.clone(),
            )
            .await;
            match result {
                Ok(()) => {}
                Err(err) => log_error_and_send_to_admin(err, bot.clone(), admin_id).await,
            };
        });
    }
}
