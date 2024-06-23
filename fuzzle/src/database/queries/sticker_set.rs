use base64::{engine::general_purpose, Engine};
use diesel::{
    delete,
    dsl::{count_star, now},
    insert_into,
    prelude::*,
    update,
    upsert::excluded,
};
use itertools::Itertools;
use tracing::warn;

use crate::{
    database::{
        query_builder::StickerTagQuery,
        schema_model::{StickerFile, StickerSet},
        Order, Sticker, StickerIdStickerFileId,
    },
    util::Emoji,
};

use super::DatabaseError;

use super::Database;

use super::super::schema::*;

define_sql_function! {
    /// Represents the `max` aggregate function for text, which doesn't make any sense, but diesel would complain otherwise
    #[aggregate]
    fn max(expr: diesel::sql_types::Text) -> diesel::sql_types::Text;
}

impl Database {
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn upsert_sticker_set_with_title(
        &self,
        id: &str,
        title: &str,
    ) -> Result<(), DatabaseError> {
        self.pool.get()?.transaction(|conn| {
            let removed: Option<String> = removed_set::table
                .filter(removed_set::id.eq(id))
                .select((removed_set::id))
                .first(conn)
                .optional()?;
            if removed.is_some() {
                return Err(DatabaseError::TryingToInsertRemovedSet);
            }
            insert_into(sticker_set::table)
                .values((sticker_set::id.eq(id), sticker_set::title.eq(title)))
                .on_conflict(sticker_set::id)
                .do_update()
                .set(sticker_set::title.eq(excluded(sticker_set::title)))
                .execute(conn)?;
            Ok(())
        })
    }

    /// title is sometimes not known immediately
    /// does not update last_fetched
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn upsert_sticker_set(
        &self,
        id: &str,
        added_by_user_id: i64,
    ) -> Result<(), DatabaseError> {
        self.pool.get()?.transaction(|conn| {
            let removed: Option<String> = removed_set::table
                .filter(removed_set::id.eq(id))
                .select((removed_set::id))
                .first(conn)
                .optional()?;
            if removed.is_some() {
                return Err(DatabaseError::TryingToInsertRemovedSet);
            }
            insert_into(sticker_set::table)
                .values((
                    sticker_set::id.eq(id),
                    sticker_set::added_by_user_id.eq(added_by_user_id),
                ))
                .on_conflict(sticker_set::id)
                .do_nothing()
                .execute(conn)?;
            Ok(())
        })
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_set_by_sticker_id(
        &self,
        sticker_id: &str,
    ) -> Result<Option<StickerSet>, DatabaseError> {
        Ok(sticker_set::table
            .filter(
                sticker_set::id.eq_any(
                    sticker::table
                        .filter(sticker::id.eq(sticker_id)) // TODO: extract common subqueries to functions returning boxed queries
                        .select((sticker::sticker_set_id)),
                ),
            )
            .select(StickerSet::as_select())
            .first(&mut self.pool.get()?)
            .optional()?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn unban_set(&self, set_id: &str) -> Result<(), DatabaseError> {
        delete(removed_set::table.filter(removed_set::id.eq(set_id)))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn ban_set(
        &self,
        set_id: &str,
        added_by_user_id: Option<i64>,
    ) -> Result<(), DatabaseError> {
        insert_into(removed_set::table)
            .values((
                removed_set::id.eq(set_id),
                removed_set::added_by_user_id.eq(added_by_user_id),
            ))
            .on_conflict_do_nothing()
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn delete_sticker_set(&self, set_id: &str) -> Result<(), DatabaseError> {
        delete(sticker_set::table.filter(sticker_set::id.eq(set_id)))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_n_least_recently_fetched_set_ids(
        &self,
        n: i64,
    ) -> Result<Vec<String>, DatabaseError> {
        Ok(sticker_set::table
            .select(sticker_set::id)
            .order_by(sticker_set::last_fetched)
            .limit(n)
            .load(&mut self.pool.get()?)?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_n_latest_sets(&self, n: i64) -> Result<Vec<StickerSet>, DatabaseError> {
        Ok(sticker_set::table
            .select(StickerSet::as_select())
            .order_by(sticker_set::created_at.desc())
            .limit(n)
            .load(&mut self.pool.get()?)?)
    }
}
