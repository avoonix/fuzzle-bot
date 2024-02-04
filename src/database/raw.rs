use serde::{Serialize, Deserialize};


use super::User;

#[derive(Debug, Serialize, Deserialize)]
// #[derive(Serialize, Deserialize, FromRow)]
pub struct RawDatabaseUser {
    // #[sqlx(try_from = "i64")]
    pub id: i64,
    pub blacklist: String,
    pub interactions: i64,
    pub can_tag_stickers: bool,
    pub can_tag_sets: bool,
}

impl TryFrom<RawDatabaseUser> for User {
    type Error = anyhow::Error; // TODO: more specific type of error?

    fn try_from(value: RawDatabaseUser) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id.try_into()?,
            blacklist: serde_json::from_str(&value.blacklist)?,
            can_tag_sets: value.can_tag_sets,
            can_tag_stickers: value.can_tag_stickers
        })
    }
}
