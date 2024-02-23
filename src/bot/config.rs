use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use teloxide::types::UserId;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub name: String,
    pub admin: String,
    pub admin_telegram_user_id: u64,
    pub telegram: Telegram,
    pub default_blacklist: Vec<String>,
    pub greeting_sticker_id: Option<String>,
    pub domain_name: String,
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub cache_dir_path: String,
    pub db_file_path: String,
    pub config_file_path: String,
}

impl Paths {
    #[must_use] pub fn config(&self) -> PathBuf {
        self.config_file_path.clone().into()
    }
    #[must_use] pub fn db(&self) -> PathBuf {
        self.db_file_path.clone().into()
    }
    #[must_use] pub fn image_cache(&self) -> PathBuf {
        format!("{}/images", self.cache_dir_path).into()
    }
    #[must_use] pub fn tag_cache(&self) -> PathBuf {
        format!("{}/tags", self.cache_dir_path).into()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Telegram {
    pub token: String,
    pub username: String,
}

impl Config {
    #[must_use]
    pub const fn get_admin_user_id(&self) -> UserId {
        UserId(self.admin_telegram_user_id)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "my bot".to_string(),
            admin: "admin".to_string(),
            admin_telegram_user_id: 0,
            telegram: Telegram::default(),
            default_blacklist: vec![
                "meta_sticker".to_string(),
                "gore".to_string(),
                "scat".to_string(),
            ],
            greeting_sticker_id: Some("AgADbRIAAhZaEFI".to_string()), // from the set t.me/addstickers/FuzzleBot
            domain_name: "ui.example.com".to_string(),
        }
    }
}

impl Default for Telegram {
    fn default() -> Self {
        Self {
            token: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_string(),
            username: "botname".to_string(),
        }
    }
}
