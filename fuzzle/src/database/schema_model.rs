use diesel::{
    backend::Backend, deserialize::FromSqlRow, expression::AsExpression, prelude::*, serialize::{self, IsNull}, sqlite::Sqlite
};
use enum_primitive_derive::Primitive;
use num_traits::{FromPrimitive, ToPrimitive};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use crate::{database::BanReason, tags::Category};

use super::{schema, DatabaseError, DialogState, ModerationTaskDetails, ModerationTaskStatus, StickerType, StringVec, UserSettings};

#[derive(Queryable, Selectable)]
#[diesel(table_name = schema::sticker_file)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct StickerFile {
    pub id: String,
    pub created_at: chrono::NaiveDateTime,
    pub tags_locked_by_user_id: Option<i64>,
    pub thumbnail_file_id: Option<String>,
    pub sticker_type: StickerType,
}

#[derive(Queryable, QueryableByName, Selectable, Debug, Clone)]
#[diesel(table_name = schema::banned_sticker)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct BannedSticker {
    pub id: String,
    pub sticker_set_id: String,
    pub telegram_file_identifier: String,
    pub sticker_file_id: String,
    pub thumbnail_file_id: Option<String>,
    pub sticker_type: StickerType,
    pub clip_max_match_distance: f32,
    pub ban_reason: BanReason,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, QueryableByName)]
#[diesel(table_name = schema::sticker_set)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct StickerSet {
    pub id: String,
    pub title: Option<String>,
    pub last_fetched: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub added_by_user_id: Option<i64>,
    pub created_by_user_id: Option<i64>,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = schema::user)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: i64,
    pub blacklist: StringVec,
    pub can_tag_stickers: bool,
    pub can_tag_sets: bool,
    pub created_at: chrono::NaiveDateTime,
    pub settings: Option<UserSettings>,
    pub dialog_state: Option<DialogState>,
}

#[derive(Queryable, QueryableByName, Selectable, Debug, Clone)]
#[diesel(table_name = schema::sticker)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Sticker {
    pub id: String,
    pub sticker_set_id: String,
    pub telegram_file_identifier: String,
    pub sticker_file_id: String,
    pub emoji: Option<String>, // TODO: custom `Emoji` type?
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = schema::sticker_file_tag)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(primary_key(sticker_file_id, tag))]
pub struct StickerFileTag {
    pub sticker_file_id: String,
    pub tag: String,
    pub added_by_user_id: Option<i64>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = schema::sticker_user)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(primary_key(sticker_id, user_id))]
pub struct StickerUser {
    pub sticker_id: String,
    pub user_id: i64,
    pub is_favorite: bool,
    pub last_used: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = schema::tag)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Tag {
    pub id: String,
    pub category: Category,
    pub created_by_user_id: Option<i64>,
    pub created_at: chrono::NaiveDateTime,
    pub linked_channel_id: Option<i64>,
    pub linked_user_id: Option<i64>,
    pub aliases: Option<StringVec>,
    pub implications: Option<StringVec>,
}

#[derive(Queryable, Selectable, Debug, Clone, QueryableByName)]
#[diesel(table_name = schema::moderation_task)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ModerationTask {
    pub id: i64,
    pub created_at: chrono::NaiveDateTime,
    pub created_by_user_id: i64,
    pub details: ModerationTaskDetails,
    pub completion_status: ModerationTaskStatus,
}
