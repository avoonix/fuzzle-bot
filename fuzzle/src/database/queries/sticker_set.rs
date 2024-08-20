use base64::{engine::general_purpose, Engine};
use diesel::{
    delete, dsl::{count_star, now, sql}, insert_into, prelude::*, sql_query, sql_types::BigInt, update, upsert::excluded
};
use itertools::Itertools;
use r2d2::PooledConnection;
use tracing::warn;

use crate::{
    database::{
        query_builder::StickerTagQuery, schema_model::{StickerFile, StickerSet}, Order, Sticker, StickerChange, StickerIdStickerFileId
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
    pub async fn create_sticker_set_with_creator(
        &self,
        id: &str,
        title: &str,
        created_by_user_id: i64,
    ) -> Result<(), DatabaseError> {
        self.pool.get()?.immediate_transaction(|conn| {
            self.check_removed(id, conn)?;
            insert_into(sticker_set::table)
                .values((
                    sticker_set::id.eq(id),
                    sticker_set::title.eq(title),
                    sticker_set::created_by_user_id.eq(created_by_user_id),
                    sticker_set::added_by_user_id.eq(created_by_user_id),
                ))
                .execute(conn)?;
            Ok(())
        })
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn upsert_sticker_set_with_title_and_creator(
        &self,
        id: &str,
        title: &str,
        created_by_user_id: i64,
        added_by_user_id: Option<i64>, // only set if the set is new, not updated
    ) -> Result<(), DatabaseError> {
        self.pool.get()?.immediate_transaction(|conn| {
            self.check_removed(id, conn)?;
            insert_into(sticker_set::table)
                .values((sticker_set::id.eq(id), sticker_set::title.eq(title), sticker_set::created_by_user_id.eq(created_by_user_id),
                sticker_set::added_by_user_id.eq(added_by_user_id)
            ))
                .on_conflict(sticker_set::id)
                .do_update()
                .set((sticker_set::title.eq(excluded(sticker_set::title)), sticker_set::created_by_user_id.eq(created_by_user_id)))
                .execute(conn)?;
            Ok(())
        })
    }

    /// title and creator is sometimes not known immediately
    /// does not update last_fetched
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn upsert_sticker_set(
        &self,
        id: &str,
        added_by_user_id: i64,
    ) -> Result<(), DatabaseError> {
        self.pool.get()?.immediate_transaction(|conn| {
            self.check_removed(id, conn)?;
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

    fn check_removed(
        &self,
        set_id: &str,
        // conn: &mut PooledConnection<diesel::r2d2::ConnectionManager<SqliteConnection>>,
        conn: &mut SqliteConnection,
    ) -> Result<(), DatabaseError> {
        let removed: Option<String> = removed_set::table
            .filter(removed_set::id.eq(set_id))
            .select((removed_set::id))
            .first(conn)
            .optional()?;
        if removed.is_some() {
            Err(DatabaseError::TryingToInsertRemovedSet)
        } else {
            Ok(())
        }
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

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_n_latest_sticker_changes(&self, n: i64) -> Result<Vec<StickerChange>, DatabaseError> {
        Ok(sql_query("select sticker.id AS sticker_id, sticker_set_id, count(case when julianday('now') - julianday(sticker_file.created_at) < 1 then true else null end) as today, count(case when julianday('now') - julianday(sticker_file.created_at) < 7 then true else null end) as this_week from sticker inner join sticker_file on sticker.sticker_file_id = sticker_file.id where julianday('now') - julianday(sticker_file.created_at) < 7 group by sticker_set_id order by max(sticker_file.created_at) desc limit 10;")
            .load(&mut self.pool.get()?)?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_owned_sticker_sets_by_bot(
        &self,
        bot_username: &str,
        user_id: i64,
    ) -> Result<Vec<StickerSet>, DatabaseError> {
        Ok(sticker_set::table
            .filter(sticker_set::created_by_user_id.eq(user_id))
            .filter(sticker_set::id.like(format!("%_by_{bot_username}")))
            .select(StickerSet::as_select())
            .load(&mut self.pool.get()?)?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_owned_sticker_sets(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<StickerSet>, DatabaseError> {
        Ok(sticker_set::table
            .filter(sticker_set::created_by_user_id.eq(user_id))
            .select(StickerSet::as_select())
            .order_by(sticker_set::created_at.desc())
            .limit(limit)
            .offset(offset)
            .load(&mut self.pool.get()?)?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_owned_sticker_set_count(
        &self,
        user_id: i64
    ) -> Result<i64, DatabaseError> {
        Ok(sticker_set::table
            .filter(sticker_set::created_by_user_id.eq(user_id))
            .select(count_star())
            .first(&mut self.pool.get()?)?)
    }

    /// returns id of sticker in the set
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn sticker_set_contains_file(
        &self,
        set_id: &str,
        file_id: &str,
    ) -> Result<Option<String>, DatabaseError> {
        Ok(sticker::table
            .filter(sticker::sticker_set_id.eq(set_id))
            .filter(sticker::sticker_file_id.eq(file_id))
            .select((sticker::id))
            .first(&mut self.pool.get()?)
            .optional()?)
    }
}
