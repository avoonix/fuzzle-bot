use std::sync::Arc;

use crate::background_tasks::{send_message_to_admin, AdminMessage, AnalysisWorker, TaggingWorker};
use crate::bot::config::Config;
use crate::database::{Database, User};
use crate::tags::TagManager;
use crate::util::Timer;
use crate::Paths;
use teloxide::prelude::*;

use super::{Bot, BotError, RequestContext};

#[derive(Clone, Debug)]
pub struct UserMeta {
    pub user: User,
    pub is_admin: bool,
}

pub async fn inject_context(
    update: Update,
    config: Arc<Config>,
    database: Database,
    tag_manager: Arc<TagManager>,
    bot: Bot,
    paths: Arc<Paths>,
    analysis_worker: AnalysisWorker,
    tagging_worker: TaggingWorker,
) -> Option<RequestContext> {
    let timer = Timer::new(update.clone());
    match get_user(
        update.clone(),
        config.clone(),
        database.clone(),
        bot.clone(),
    )
    .await
    {
        Ok(user) => Some(RequestContext {
            bot,
            config,
            database,
            tag_manager,
            user: Arc::new(user),
            paths,
            timer: Arc::new(timer),
            analysis_worker,
            tagging_worker,
        }),
        Err(err) => {
            // TODO: handle error
            drop(err);
            None
        }
    }
}

async fn get_user(
    update: Update,
    config: Arc<Config>,
    database: Database,
    bot: Bot,
) -> Result<UserMeta, BotError> {
    // TODO: possibly cache users? TODO: measure how long this function takes
    let Some(user) = update.user() else {
        return Err(anyhow::anyhow!("user missing from telegram update"))?;
    };

    get_or_create_user(user.id, config, database, bot).await
}

pub async fn get_or_create_user(
    user_id: UserId,
    config: Arc<Config>,
    database: Database,
    bot: Bot,
) -> Result<UserMeta, BotError> {
    let user = database.get_user(user_id.0).await?;
    let user = match user {
        Some(user) => user,
        None => {
            send_message_to_admin(
                AdminMessage::NewUser,
                user_id,
                bot,
                config.get_admin_user_id(),
            )
            .await?;
            database.create_user(user_id.0, config.default_blacklist.clone()).await?
        }
    };

    // TODO: check if user is banned?
    let is_admin = user_id == config.get_admin_user_id();
    Ok(UserMeta { user, is_admin })
}

impl UserMeta {
    #[must_use]
    pub const fn can_tag_stickers(&self) -> bool {
        self.user.can_tag_stickers
    }

    #[must_use]
    pub const fn can_tag_sets(&self) -> bool {
        self.user.can_tag_sets
    }

    #[must_use]
    pub const fn id(&self) -> UserId {
        UserId(self.user.id)
    }
}
