use log::warn;
use serde::{Deserialize, Serialize};

use super::{SqlDateTime, User, UserSettings};

#[derive(Debug, Serialize, Deserialize)]
// #[derive(Serialize, Deserialize, FromRow)]
pub struct RawDatabaseUser {
    // #[sqlx(try_from = "i64")]
    pub id: i64,
    pub blacklist: String,
    pub settings: Option<String>,
    pub interactions: i64,
    pub can_tag_stickers: bool,
    pub can_tag_sets: bool,
    pub created_at: SqlDateTime,
}

impl TryFrom<RawDatabaseUser> for User {
    type Error = anyhow::Error; // TODO: more specific type of error?

    fn try_from(value: RawDatabaseUser) -> Result<Self, Self::Error> {
        let settings = match value.settings {
            Some(settings) => 
                match serde_json::from_str(&settings) {
                    Err(err) => {
                        warn!("settings parse error: {}", err);
                        UserSettings::default()
                    }
                    Ok(settings) => settings,
                }
            
            None => UserSettings::default(),
        };

        Ok(Self {
            id: value.id.try_into()?,
            blacklist: serde_json::from_str(&value.blacklist)?,
            can_tag_sets: value.can_tag_sets,
            can_tag_stickers: value.can_tag_stickers,
            settings,
        })
    }
}
