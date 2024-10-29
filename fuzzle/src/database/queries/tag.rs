use diesel::delete;
use diesel::dsl::count_star;
use diesel::dsl::exists;
use diesel::dsl::not;
use diesel::dsl::sql;
use diesel::insert_into;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::BigInt;
use diesel::sql_types::Text;
use itertools::Itertools;
use std::collections::HashMap;
use teloxide::types::ChatId;
use teloxide::types::UserId;

use crate::database::model::PopularTag;
use crate::database::StringVec;
use crate::database::Tag;
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
        let tag_id = tag_id.to_string();
        self.pool
            .exec(move |conn| {
                Ok(tag::table
                    .filter(tag::id.eq(tag_id))
                    .select(Tag::as_select())
                    .first(conn)
                    .optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_all_tags(&self) -> Result<Vec<Tag>, DatabaseError> {
        self.pool
            .exec(move |conn| Ok(tag::table.select(Tag::as_select()).load(conn)?))
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_all_tags_by_linked_user_id(
        &self,
        user_id: i64,
    ) -> Result<Vec<Tag>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(tag::table
                    .filter(tag::linked_user_id.eq(user_id))
                    .select(Tag::as_select())
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn delete_tag(&self, tag_id: &str) -> Result<(), DatabaseError> {
        let tag_id = tag_id.to_string();
        self.pool
            .exec(move |conn| {
                delete(tag::table.filter(tag::id.eq(tag_id))).execute(conn)?;
                Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn upsert_tag(
        &self,
        tag_id: &str,
        category: Category,
        created_by_user_id: i64,
        linked_channel_id: Option<i64>,
        linked_user_id: Option<i64>,
        aliases: Vec<String>,
        implications: Vec<String>,
    ) -> Result<(), DatabaseError> {
        let tag_id = tag_id.to_string();
        self.pool
            .exec(move |conn| {
                let aliases = (!aliases.is_empty()).then(|| StringVec::from(aliases));
                let implications =
                    (!implications.is_empty()).then(|| StringVec::from(implications));

                insert_into(tag::table)
                    .values((
                        tag::id.eq(tag_id),
                        tag::category.eq(category),
                        tag::created_by_user_id.eq(created_by_user_id),
                        tag::linked_channel_id.eq(linked_channel_id),
                        tag::linked_user_id.eq(linked_user_id),
                        tag::aliases.eq(&aliases),
                        tag::implications.eq(&implications),
                    ))
                    .on_conflict(tag::id)
                    .do_update()
                    .set((
                        tag::category.eq(category),
                        tag::created_by_user_id.eq(created_by_user_id),
                        tag::linked_channel_id.eq(linked_channel_id),
                        tag::linked_user_id.eq(linked_user_id),
                        tag::aliases.eq(&aliases),
                        tag::implications.eq(&implications),
                    ))
                    .execute(conn)?;
                Ok(())
            })
            .await
    }
}
