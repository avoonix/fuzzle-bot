use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use teloxide::types::UserId;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub cache_dir_path: String,
    pub db_file_path: String,

    pub bot_display_name: String, // TODO: use this instead of hardcoding
    pub greeting_sticker_id: Option<String>,
    pub default_blacklist: Vec<String>,

    pub vector_db_url: String,
    pub inference_url: String,

    pub domain_name: String,
    pub http_listen_address: String,

    pub admin_telegram_user_id: u64,
    pub telegram_bot_token: String,
    pub telegram_bot_username: String,
}

impl Config {
    #[must_use]
    pub const fn get_admin_user_id(&self) -> UserId {
        UserId(self.admin_telegram_user_id)
    }

    #[must_use]
    pub fn db(&self) -> PathBuf {
        self.db_file_path.clone().into()
    }

    #[must_use]
    pub fn tag_cache(&self) -> PathBuf {
        format!("{}/tags", self.cache_dir_path).into()
    }
}
