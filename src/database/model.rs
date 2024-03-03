use std::{collections::HashMap};

use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sqlx::prelude::FromRow;

use crate::util::Emoji;

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
    pub is_animated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub created_at: SqlDateTime,
    pub sticker_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct FileAnalysis {
    pub id: String,
    pub thumbnail_file_id: Option<String>,
    pub visual_hash: Option<Vec<u8>>,
    pub histogram: Option<Vec<u8>>,
    pub embedding: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct FileAnalysisWithStickerId {
    pub id: String,
    pub thumbnail_file_id: Option<String>,
    pub visual_hash: Option<Vec<u8>>,
    pub histogram: Option<Vec<u8>>,
    pub embedding: Option<Vec<u8>>,
    pub sticker_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub sets: i64,
    pub stickers: i64,
    pub taggings: i64,
    pub tagged_stickers: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct AdminStats {
    pub number_of_sets_fetched_in_24_hours: i64,
    pub least_recently_fetched_set_age: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct FullUserStats {
    pub total_tagged: i64,
    pub total_untagged: i64,
    pub tagged_24hrs: i64,
    pub untagged_24hrs: i64,
    pub sets: HashMap<String, AddedRemoved>,
}

#[derive(Debug, Clone, Copy)]
pub struct AddedRemoved {
    pub added: i64,
    pub removed: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub blacklist: Vec<String>,
    pub settings: UserSettings,
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
    pub file_hash: String,
    pub emoji: Option<Emoji>, // TODO: every sticker has an emoji
    pub set_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserSettings {
    pub order: Option<StickerOrder>
}

impl UserSettings {
    pub fn order(&self) -> StickerOrder {
        self.order.unwrap_or_default()
    }
}

#[derive(Debug, Serialize_repr, Deserialize_repr, Clone, Default, PartialEq, Eq, Copy)]
#[repr(u8)]
pub enum StickerOrder {
    #[default]
    LatestFirst = 0,
    Random = 1,
}
