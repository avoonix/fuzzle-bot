use teloxide::types::UserId;

use super::{Bot, BotError};
use crate::{background_tasks::TaggingWorker, database::{Database, DialogState, User}, qdrant::VectorDatabase, tags::TagManager, util::Required, Config};
use std::sync::Arc;

#[derive(Clone)]
pub struct RequestContext {
    pub config: Arc<Config>,
    pub database: Database,
    pub tag_manager: Arc<TagManager>,
    pub bot: Bot,
    pub user: Arc<User>,
    pub tagging_worker: TaggingWorker,
    // pub tag_worker: TagWorker,
    pub vector_db: VectorDatabase,
}

impl RequestContext {
    #[must_use]
    pub fn can_tag_stickers(&self) -> bool {
        self.user.can_tag_stickers
    }

    #[must_use]
    pub fn can_tag_sets(&self) -> bool {
        self.user.can_tag_sets
    }

    pub fn user_id(&self) -> UserId {
        UserId(self.user.id as u64)
    }
    pub fn is_admin(&self) -> bool {
        self.user_id() == self.config.get_admin_user_id()
    }
    pub fn dialog_state(&self) -> DialogState {
        self.user.dialog_state.clone().unwrap_or_default()
    }
    pub fn is_continuous_tag_state(&self) -> bool {
            matches!(self.dialog_state(), DialogState::ContinuousTag { .. })
    }
    pub fn is_recommender_state(&self) -> bool {
            matches!(self.dialog_state(), DialogState::StickerRecommender { .. })
    }
    pub async fn with_updated_user(&self) -> Result<RequestContext, BotError> {
        let mut new_context = self.clone();
        new_context.user = Arc::new(self.database.get_user_by_id(self.user.id).await?.required()?);
        Ok(new_context)
    }
}
