use diesel::delete;
use diesel::dsl::count_star;
use diesel::dsl::exists;
use diesel::dsl::not;
use diesel::dsl::sql;
use diesel::insert_into;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use diesel::sql_query;
use diesel::sql_types::BigInt;
use diesel::sql_types::Text;
use itertools::Itertools;
use r2d2::PooledConnection;
use std::collections::HashMap;
use teloxide::types::ChatId;
use teloxide::types::UserId;

use crate::database::model::PopularTag;
use crate::database::Tag;
use crate::database::TagData;
use crate::database::UserStats;
use crate::tags::Category;
use crate::util::Emoji;

use super::sticker::max;
use super::DatabaseError;

use super::Database;

use super::super::schema::*;

impl Database {
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_tag_by_id(&self, tag_id: &str) -> Result<Option<Tag>, DatabaseError> {
        Ok(tag::table
            .filter(tag::id.eq(tag_id))
            .select(Tag::as_select())
            .first(&mut self.pool.get()?)
            .optional()?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn all_approved_tags(&self, sticker_id: &str) -> Result<Vec<Tag>, DatabaseError> {
        Ok(tag::table
            .filter(tag::is_pending.eq(false))
            .select(Tag::as_select())
            .load(&mut self.pool.get()?)?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn create_pending_tag(
        &self,
        tag_id: &str,
        category: Category,
        linked_channel: &Option<(ChatId, String)>,
        linked_user: &Option<(UserId, String)>,
        example_sticker_ids: &[String],
        aliases: &[String],
        user_id: i64,
    ) -> Result<(), DatabaseError> {
        insert_into(tag::table)
            .values((
                tag::id.eq(tag_id),
                tag::category.eq(category),
                tag::is_pending.eq(true),
                tag::created_by_user_id.eq(user_id),
                tag::dynamic_data.eq(Some(TagData {
                    aliases: aliases.into(),
                    example_sticker_ids: example_sticker_ids.into(),
                    linked_channel: linked_channel.clone(),
                    linked_user: linked_user.clone(),
                })),
            ))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    // TODO: (wizard) check if tag request already exists for current tag_id
    // TODO: create tag
    // TODO: delete tag (admin only)
    // TODO: approve tag (remove )
    // TODO: list tags

    // #[tracing::instrument(skip(self), err(Debug))]
    // pub async fn delete_sticker(&self, sticker_id: &str) -> Result<(), DatabaseError> {
    //     delete(sticker::table.filter(sticker::id.eq(sticker_id))).execute(&mut self.pool.get()?)?;
    //     Ok(())
    // }
}
