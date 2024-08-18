use crate::bot::config::Config;
use crate::callback::callback_handler_wrapper;
use crate::database::Database;
use crate::inline::{inline_query_handler_wrapper, inline_result_handler_wrapper};
use crate::message::{list_visible_admin_commands, list_visible_user_commands, message_handler_wrapper};
use crate::qdrant::VectorDatabase;
use crate::tags::{get_default_tag_manager, TagManager};

use crate::background_tasks::{start_periodic_tasks, TaggingWorker, Worker};

use std::sync::Arc;
use std::time::Duration;
use teloxide::adaptors::throttle::Limits;
use teloxide::net::default_reqwest_settings;
use teloxide::prelude::*;
use teloxide::types::{AllowedUpdate, ParseMode, UpdateKind};
use teloxide::update_listeners::Polling;
use tracing::{info, warn};
use url::Url;

use super::error_handler::ErrorHandler;
use super::user_meta::inject_context;
use super::{Bot, InternalError};

pub struct UpdateListener {
    tags: Arc<TagManager>,
    bot: Bot,
    config: Arc<Config>,
    database: Database,
    vector_db: VectorDatabase,
}

impl UpdateListener {
    #[tracing::instrument(name="UpdateListener::new", skip(config), err(Debug))]
    pub async fn new(config: Config) -> Result<Self, anyhow::Error> {
        let tags = get_default_tag_manager(config.tag_cache()).await?;
        let client = default_reqwest_settings()
            .timeout(Duration::from_secs(30))
            .build()?;
        // let bot = Bot::with_client(scrape_config.token, client);
        let bot = teloxide::Bot::with_client(config.telegram_bot_token.clone(), client)
            .set_api_url(Url::parse(&config.telegram_bot_api_url).unwrap())
            .parse_mode(ParseMode::MarkdownV2)
            .throttle(Limits::default());
        let database = Database::new(config.db()).await?;
        let vector_db = VectorDatabase::new(&config.vector_db_url).await?;
        let config = Arc::new(config);

        Ok(Self {
            tags,
            bot,
            config,
            database,
            vector_db,
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
        let tagging_worker = TaggingWorker::start(self.database.clone(), Arc::clone(&self.tags));
        // let tag_worker = TagWorker::start(self.database.clone(), Arc::clone(&self.tags));
        start_periodic_tasks(
            self.bot.clone(),
            self.database.clone(),
            self.config.clone(),
            self.tags.clone(),
            self.vector_db.clone(),
        );

        crate::web::server::setup(
            self.config.clone(),
            self.database.clone(),
            self.tags.clone(),
            self.bot.clone(),
            tagging_worker.clone(),
            // tag_worker.clone(),
            self.vector_db.clone(),
        );

        let handler: Handler<'_, _, Result<(), ()>, _> = dptree::entry()
            .chain(dptree::filter_map_async(inject_context))
            .branch(Update::filter_message().endpoint(message_handler_wrapper))
            .branch(Update::filter_callback_query().endpoint(callback_handler_wrapper))
            .branch(Update::filter_inline_query().endpoint(inline_query_handler_wrapper))
            .branch(Update::filter_chosen_inline_result().endpoint(inline_result_handler_wrapper));

        let update_listener = Polling::builder(self.bot.clone())
            .timeout(Duration::from_secs(10))
            .allowed_updates(vec![
                AllowedUpdate::Message,
                AllowedUpdate::InlineQuery,
                AllowedUpdate::ChosenInlineResult,
                AllowedUpdate::CallbackQuery,
                AllowedUpdate::MyChatMember,
            ])
            .delete_webhook()
            .await
            .build();

        info!("Listening ...");
        Dispatcher::builder(self.bot.clone(), handler)
            .dependencies(dptree::deps![
                self.config.clone(),
                self.tags.clone(),
                self.database.clone(),
                tagging_worker,
                // tag_worker,
                self.vector_db.clone()
            ])
            .default_handler(|upd| async move {
                let span = tracing::error_span!("default_handler").entered();
                tracing::error!("Unhandled update: {:?}", upd);
                span.exit();
            })
            .error_handler(ErrorHandler::new())
            .distribution_function(distribution_function)
            .worker_queue_size(621)
            .enable_ctrlc_handler()
            .build()
            .dispatch_with_listener(
                update_listener,
                LoggingErrorHandler::with_custom_text("UPDATE LISTENER ERROR"),
            )
            .await;
        Ok(())
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
struct DistributionKey(Option<ChatId>, DistributionUpdateKind);

/// each update kind gets its own queue
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
enum DistributionUpdateKind {
    // StickerMessage,
    // OtherMessage,
    // InlineQuery,
    ChosenInlineResult,
    // CallbackQuery,
    Other,
}

fn distribution_function(update: &Update) -> Option<DistributionKey> {
    let chat_id = update.chat().map(|c| c.id);
    let kind = match update.kind {
        // all operations that don't depend on order can get their own queue (due to dialog state, most previous update kinds are no longer independent)
        // TODO: we can still split updates, but we need to check the exact operation and only put independent events in different queues

        // UpdateKind::Message(ref message) => {
        //     if message.sticker().is_some() {
        //         DistributionUpdateKind::StickerMessage
        //     } else {
        //         DistributionUpdateKind::OtherMessage
        //     }
        // }
        // UpdateKind::InlineQuery(..) => DistributionUpdateKind::InlineQuery,
        // UpdateKind::CallbackQuery(..) => DistributionUpdateKind::CallbackQuery,
        UpdateKind::ChosenInlineResult(..) => DistributionUpdateKind::ChosenInlineResult,
        _ => DistributionUpdateKind::Other,
    };
    Some(DistributionKey(chat_id, kind))
}
