use std::sync::Arc;

use crate::background_tasks::{TagManagerService, TfIdfService};
use crate::bot::config::Config;
use crate::database::{Database, User};
use crate::qdrant::VectorDatabase;
use crate::services::Services;
use itertools::Itertools;
use teloxide::prelude::*;
use teloxide::types::ChatKind;

use super::{Bot, BotError, RequestContext};

#[tracing::instrument(skip(update, config, database, tag_manager, bot, tfidf_service, vector_db, services))]
pub async fn inject_context(
    update: Update,
    config: Arc<Config>,
    database: Database,
    tag_manager: TagManagerService,
    bot: Bot,
    tfidf_service: TfIdfService,
    vector_db: VectorDatabase,
    services: Services,
) -> Option<RequestContext> {
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
            tfidf: tfidf_service,
            vector_db,
            services,
        }),
        Err(err) => {
            tracing::error!("error during inject: {err}");
            None
        }
    }
}

#[tracing::instrument(skip(config, database, bot, update), err(Debug))]
async fn get_user(
    update: Update,
    config: Arc<Config>,
    database: Database,
    bot: Bot,
) -> Result<User, BotError> {
    let Some(user_id) = (match (update.from(), update.chat()) {
        (Some(user), _) => Some(user.id),
        (None, Some(chat)) => chat.id.as_user(),
        (None, None) => None,
    }) else {
        return Err(anyhow::anyhow!("user missing from telegram update").into());
    };

    get_or_create_user(user_id, config, database, bot).await
}

pub async fn get_or_create_user(
    user_id: UserId,
    config: Arc<Config>,
    database: Database,
    bot: Bot,
) -> Result<User, BotError> {
    let user = database.get_user_by_id(user_id.0 as i64).await?;
    let user = match user {
        Some(user) => user,
        None => {
            database
                .create_user(user_id.0 as i64, config.default_blacklist.clone().into())
                .await?
        }
    };

    // TODO: check if user is banned?
    Ok(user)
}
