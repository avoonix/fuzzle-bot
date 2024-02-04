use chrono::Duration;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

// type SqlDateTime = chrono::DateTime<chrono::Utc>;
pub type SqlDateTime = chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct Relationship {
    #[serde(rename = "in")]
    pub in_: String,
    pub out: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PopularTag {
    pub name: String,
    pub count: u64,
}


#[derive(Debug, Serialize, Deserialize, Copy, Clone, Default)]
pub struct UserStats {
    pub added_tags: i64,
    pub removed_tags: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct StickerSet<'a> {
    pub name: &'a str,
    pub title: &'a str,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct SavedStickerSet {
    pub id: String,
    pub title: Option<String>,
    // pub last_fetched: chrono::DateTime<chrono::Utc>
    // pub last_fetched: chrono::NaiveDateTime,
    pub last_fetched: Option<SqlDateTime>,
    pub created_at: SqlDateTime,
}

#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub sets: i64,
    pub stickers: i64,
    pub taggings: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct AdminStats {
    pub number_of_sets_fetched_in_24_hours: i64,
    pub least_recently_fetched_set_age: Option<Duration>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub blacklist: Vec<String>,

    pub can_tag_sets: bool,
    pub can_tag_stickers: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct Sticker<'a> {
    pub file_id: &'a str,
    pub unique_id: &'a str,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedSticker {
    pub id: String,
    pub file_id: String,
}
