use chrono::Duration;
use diesel::sql_types::BigInt;
use diesel::{
    backend::Backend,
    deserialize::{self, FromSqlRow},
    expression::AsExpression,
    prelude::*,
    serialize::{self, IsNull},
    sqlite::Sqlite,
};
use enum_primitive_derive::Primitive;
use num_traits::{FromPrimitive, ToPrimitive};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};
use teloxide::types::{ChatId, UserId};
use teloxide::{requests::Requester, types::InputSticker};

use crate::{bot::Bot, tags::Category, util::Emoji};

#[derive(Debug, Serialize, Deserialize)]
pub struct PopularTag {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, Default)]
pub struct UserStats {
    pub added_tags: i64,
    pub removed_tags: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub sets: i64,
    pub stickers: i64,
    pub taggings: i64,
    pub tagged_stickers: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct PersonalStats {
    pub favorites: i64,
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

#[derive(Debug, Clone, Copy, Default)]
pub struct AddedRemoved {
    pub added: i64,
    pub removed: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserSettings {
    pub order: Option<StickerOrder>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TagData {
    pub example_sticker_ids: Vec<String>,
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_channel: Option<(ChatId, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_user: Option<(UserId, String)>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct StickerIdStickerFileId {
    pub sticker_id: String,
    pub sticker_file_id: String,
}

macro_rules! json_wrapper {
    ($wrapper:ident, $wrappee:ty) => {
        #[derive(AsExpression, FromSqlRow, Debug, Clone, Serialize, Deserialize)]
        // #[diesel(sql_type = VarChar)]
        #[diesel(sql_type = diesel::sql_types::Text)]
        // #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct $wrapper($wrappee);

        impl Deref for $wrapper {
            type Target = $wrappee;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $wrapper {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl $wrapper {
            pub fn into_inner(self) -> $wrappee {
                self.0
            }
        }

        impl From<$wrappee> for $wrapper {
            fn from(value: $wrappee) -> Self {
                Self(value)
            }
        }
    };
}

json_wrapper!(Blacklist, Vec<String>);

macro_rules! impl_json {
    ($type_name:ty) => {
        impl serialize::ToSql<diesel::sql_types::Text, Sqlite> for $type_name
        where
            String: serialize::ToSql<diesel::sql_types::Text, Sqlite>,
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut serialize::Output<'b, '_, Sqlite>,
            ) -> serialize::Result {
                let serialized = serde_json::to_string(&self)?;
                out.set_value(serialized);
                Ok(IsNull::No)
            }
        }

        impl<B: Backend> deserialize::FromSql<diesel::sql_types::Text, B> for $type_name
        where
            String: deserialize::FromSql<diesel::sql_types::Text, B>,
        {
            fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
                let string_value =
                    <String as deserialize::FromSql<diesel::sql_types::Text, B>>::from_sql(bytes)?;
                Ok(serde_json::from_str(&string_value)?)
            }
        }
    };
}

impl_json!(UserSettings);
impl_json!(TagData);
impl_json!(Blacklist);
impl_json!(DialogState);

#[derive(PartialEq, Debug, Copy, Clone, Primitive, AsExpression)]
#[diesel(sql_type = diesel::sql_types::BigInt)]
pub enum MergeStatus {
    Queued = 0,
    Merged = 1,
    NotMerged = 2,
}

#[derive(PartialEq, Debug, Copy, Clone, Primitive, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::BigInt)]
pub enum StickerType {
    Animated = 0,
    Video = 1,
    Static = 2,
}

macro_rules! impl_enum {
    ($type_name:ty) => {
        impl serialize::ToSql<diesel::sql_types::BigInt, Sqlite> for $type_name
        where
            i64: serialize::ToSql<diesel::sql_types::BigInt, Sqlite>,
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut serialize::Output<'b, '_, Sqlite>,
            ) -> serialize::Result {
                let serialized = self.to_i64();
                out.set_value(serialized);
                Ok(IsNull::No)
            }
        }

        impl<B: Backend> deserialize::FromSql<diesel::sql_types::BigInt, B> for $type_name
        where
            i64: deserialize::FromSql<diesel::sql_types::BigInt, B>,
        {
            fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
                let i64_value =
                    <i64 as deserialize::FromSql<diesel::sql_types::BigInt, B>>::from_sql(bytes)?;
                Ok(Self::from_i64(i64_value)
                    .ok_or_else(|| anyhow::anyhow!("could not convert enum"))?)
            }
        }
    };
}

impl_enum!(MergeStatus);
impl_enum!(StickerType);
impl_enum!(Category);

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub enum DialogState {
    #[default]
    Normal,
    ContinuousTag {
        #[serde(default)]
        add_tags: Vec<String>,
        #[serde(default)]
        remove_tags: Vec<String>,
    },
    StickerRecommender {
        #[serde(default)]
        positive_sticker_id: Vec<String>,
        #[serde(default)]
        negative_sticker_id: Vec<String>,
    },
    TagCreator(TagCreator),
    // TODO: use
    //     bot.set_sticker_set_thumb(name, user_id); // as soon as the sticker pack has 4 stickers -> thumnail: first 3 stickers + fuzzle bot icon
    //     // not existing yet: setStickerEmojiList, setStickerKeywords
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TagCreator {
    pub tag_id: String,

    #[serde(default)]
    pub linked_channel: Option<(ChatId, String)>,
    #[serde(default)]
    pub linked_user: Option<(UserId, String)>,

    #[serde(default)]
    pub category: Option<Category>,
    #[serde(default)]
    pub example_sticker_id: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    // TODO: when transitioning from/to other modes, keep some of the data
    // eg example_sticker_id could become positive_sticker_id
}

// TODO: test that defaults work
// - database null -> Normal
// - database ContinuousTag {} -> defaults for add_tag and remove_tag

#[derive(QueryableByName, Debug, Clone)]
pub struct StickerChange {
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub sticker_id: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub sticker_set_id: String,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub today: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub this_week: i64,
}


#[derive(QueryableByName, Debug, Clone)]
pub struct UserStickerStat {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub user_id: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub set_count: i64,
}
