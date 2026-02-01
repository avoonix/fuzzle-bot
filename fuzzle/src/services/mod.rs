mod telegram_external;

use std::sync::Arc;

pub use telegram_external::*;

use crate::Config;

pub struct Services {
    pub telegram: ExternalTelegramService
}

impl Services {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            telegram: ExternalTelegramService::new(&config.external_telegram_service_base_url),
        }
 
    }
}

