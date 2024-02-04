use crate::bot::config::Config;
use crate::callback::callback_handler;
use crate::database::Database;
use crate::inline::{inline_query_handler, inline_result_handler};
use crate::message::{list_visible_admin_commands, list_visible_user_commands, message_handler};
use crate::tags::{get_default_tag_manager, TagManager};
use crate::worker::WorkerPool;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use teloxide::adaptors::throttle::Limits;
use teloxide::net::default_reqwest_settings;
use teloxide::prelude::*;
use teloxide::types::{AllowedUpdate, ParseMode};
use teloxide::update_listeners::Polling;

use super::error_handler::ErrorHandler;
use super::user_meta::inject_user;
use super::{Bot, BotError};

#[derive(Debug)]
pub struct UpdateListener {
    tags: Arc<TagManager>,
    bot: Bot,
    config: Config,
    database: Database,
}

impl UpdateListener {
    pub async fn new(
        config: Config,
        tag_dir: PathBuf,
        database_path: PathBuf,
    ) -> Result<Self, anyhow::Error> {
        let tags = get_default_tag_manager(tag_dir).await?;
        let client = default_reqwest_settings()
            .timeout(Duration::from_secs(30))
            .build()?;
        // let bot = Bot::with_client(scrape_config.token, client);
        let bot = teloxide::Bot::with_client(config.telegram.token.clone(), client)
            .throttle(Limits::default())
            .parse_mode(ParseMode::MarkdownV2);
        let database = Database::new(database_path).await?;

        Ok(Self {
            tags,
            bot,
            config,
            database,
        })
    }

    pub async fn setup_buttons(&self) -> anyhow::Result<()> {
        self.bot
            .set_my_commands(list_visible_user_commands())
            .await?;
        // convert admin_telegram_user_id to i64
        self.bot
            .set_my_commands(list_visible_admin_commands())
            .scope(teloxide::types::BotCommandScope::Chat {
                chat_id: self.config.get_admin_user_id().into(),
            })
            .await?;
        self.bot
            .set_chat_menu_button()
            .menu_button(teloxide::types::MenuButton::Commands)
            .await?;

        Ok(())
    }

    pub async fn listen(&self) -> anyhow::Result<()> {
        let (handle, worker) = WorkerPool::start_manager(
            self.bot.clone(),
            self.config.get_admin_user_id(),
            self.database.clone(),
            self.config.worker.queue_length,
            self.config.worker.concurrency,
        )
        .await;

        let handler: Handler<'_, _, Result<(), BotError>, _> = dptree::entry()
            .chain(dptree::filter_map_async(inject_user))
            .branch(Update::filter_message().endpoint(message_handler))
            .branch(Update::filter_callback_query().endpoint(callback_handler))
            .branch(Update::filter_inline_query().endpoint(inline_query_handler))
            .branch(Update::filter_chosen_inline_result().endpoint(inline_result_handler));

        let update_listener = Polling::builder(self.bot.clone())
            .timeout(Duration::from_secs(10))
            .allowed_updates(vec![
                AllowedUpdate::Message,
                AllowedUpdate::InlineQuery,
                AllowedUpdate::ChosenInlineResult,
                AllowedUpdate::CallbackQuery,
                AllowedUpdate::MyChatMember,
            ])
            .delete_webhook().await 
            .build();

        log::info!("Listening ...");
        Dispatcher::builder(self.bot.clone(), handler)
            .dependencies(dptree::deps![
                self.config.clone(),
                self.tags.clone(),
                worker,
                self.database.clone()
            ])
            .default_handler(|upd| async move {
                log::warn!("Unhandled update: {:?}", upd);
            })
            .error_handler(ErrorHandler::new( self.bot.clone(), self.config.get_admin_user_id()))
            .enable_ctrlc_handler()
            .build()
            .dispatch_with_listener(
                update_listener,
                    LoggingErrorHandler::with_custom_text("UPDATE LISTENER ERROR"),
            )
            .await;
        drop(handle);
        Ok(())
    }
}
