use serde::{Deserialize, Serialize};
use teloxide::types::UserId;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub name: String,
    pub admin: String,
    pub admin_telegram_user_id: u64,
    pub telegram: Telegram,
    pub worker: Worker,
    pub default_blacklist: Vec<String>,
    pub greeting_sticker_id: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Telegram {
    pub token: String,
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Worker {
    pub queue_length: usize,
    pub concurrency: usize,
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
            worker: Worker::default(),
            default_blacklist: vec![
                "meta_sticker".to_string(),
                "gore".to_string(),
                "scat".to_string(),
            ],
            greeting_sticker_id: Some("AgADbRIAAhZaEFI".to_string()), // t.me/addstickers/FuzzleBot
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

impl Default for Worker {
    fn default() -> Self {
        Self {
            queue_length: 128,
            concurrency: 4,
        }
    }
}
