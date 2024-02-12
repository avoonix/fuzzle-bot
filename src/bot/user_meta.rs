use crate::bot::config::Config;
use crate::database::{Database, User};
use crate::worker::{AdminMessage, WorkerPool};
use teloxide::prelude::*;

use super::BotError;

#[derive(Clone, Debug)]
pub struct UserMeta {
    pub user: User,
    pub is_admin: bool,
}

pub async fn inject_user(
    update: Update,
    config: Config,
    database: Database,
    worker: WorkerPool,
) -> Option<UserMeta> {
    match _inject_user(update, config, database, worker).await {
        Ok(user) => Some(user),
        Err(err) => {
            // TODO: handle error
            dbg!(err);
            None
        }
    }
}

// TODO: rename method
async fn _inject_user(
    update: Update,
    config: Config,
    database: Database,
    worker: WorkerPool,
) -> Result<UserMeta, BotError> {
    // TODO: possibly cache users? TODO: measure how long this function takes
    let Some(user) = update.user() else {
        return Err(anyhow::anyhow!("user missing from telegram update"))?;
    };

    get_or_create_user(user.id, config, database, worker).await
}

pub async fn get_or_create_user(
    user_id: UserId,
    config: Config,
    database: Database,
    worker: WorkerPool,
) -> Result<UserMeta, BotError> {
    let user_exists = database.has_user(user_id.0).await?;
    if !user_exists {
        worker
            .dispatch_message_to_admin(user_id, AdminMessage::NewUser)
            .await;
    }

    let is_admin = user_id == config.get_admin_user_id();
    let user = database
        .add_user_if_not_exist(user_id.0, config.default_blacklist)
        .await?;

    // TODO: check if user is banned?

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
