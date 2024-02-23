use teloxide::types::UserId;

use super::{Bot, UserMeta};
use crate::{background_tasks::{AnalysisWorker, TaggingWorker}, database::Database, tags::TagManager, util::Timer, Config, Paths};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub config: Arc<Config>,
    pub database: Database,
    pub tag_manager: Arc<TagManager>,
    pub bot: Bot,
    pub user: Arc<UserMeta>,
    pub paths: Arc<Paths>,
    pub timer: Arc<Timer>,
    pub analysis_worker: AnalysisWorker,
    pub tagging_worker: TaggingWorker,
}

impl RequestContext {
    pub fn user_id(&self) -> UserId {
        self.user.id()
    }
    pub fn is_admin(&self) -> bool {
        self.user.is_admin
    }
}
