use std::sync::Arc;

use futures::future::BoxFuture;
use log::error;
use teloxide::prelude::*;
use teloxide::types::Recipient;

use crate::{bot::BotExt, text::Markdown};

use super::{Bot, BotError};

#[derive(Debug)]
pub struct ErrorHandler {
    bot: Bot,
    admin_id: UserId,
}

impl ErrorHandler {
    #[must_use]
    pub(crate) fn new(bot: Bot, admin_id: UserId) -> Arc<Self> {
        Arc::new(Self { bot, admin_id })
    }
}

impl teloxide::error_handlers::ErrorHandler<BotError> for ErrorHandler {
    fn handle_error(self: Arc<Self>, error: BotError) -> BoxFuture<'static, ()> {
        log::error!("Error: {:?}", error);

        let error = format!("{error:?}");

        Box::pin(async move {
            let mut message = format!("Error: {error}");
            message.truncate(3000);
            let result = self
                .bot
                .send_markdown(
                    Recipient::Id(self.admin_id.into()),
                    Markdown::escaped(message),
                )
                .await;
            match result {
                Ok(_) => {}
                Err(err) => {
                    error!("Error: {err}");
                }
            }
        })
    }
}

pub async fn log_error_and_send_to_admin(error: BotError, bot: Bot, admin_id: UserId) {
    let mut error = format!("{error:?}");
    error!("Error: {error}");
    error.truncate(3000); // telegram message length limit: 4096
    let result = bot
        .send_markdown(Recipient::Id(admin_id.into()), Markdown::escaped(error))
        .await;
    match result {
        Ok(_) => {}
        Err(err) => {
            error!("Error: {err:?}");
        }
    }
}
