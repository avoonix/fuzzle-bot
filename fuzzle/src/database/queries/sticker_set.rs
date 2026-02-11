use base64::{engine::general_purpose, Engine};
use chrono::NaiveDateTime;
use diesel::{
    debug_query, delete, dsl::{count_star, now, sql}, insert_into, prelude::*, query_builder::{BoxedSqlQuery, SqlQuery}, sql_query, sql_types::BigInt, sqlite::Sqlite, update, upsert::excluded
};
use itertools::Itertools;
use tracing::warn;

use crate::{
    database::{
        query_builder::StickerTagQuery,
        schema_model::{StickerFile, StickerSet},
        Order, Sticker, StickerChange, StickerIdStickerFileId,
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
        set_id: &str,
        title: &str,
        added_by_user_id: Option<i64>, // only set if the set is new, not updated
    ) -> Result<(), DatabaseError> {
        let set_id = set_id.to_string();
        let title = title.to_string();
        self.pool
            .exec(move |conn| {
                conn.immediate_transaction(|conn| {
                    Self::check_removed(&set_id, conn)?;
                    insert_into(sticker_set::table)
                        .values((
                            sticker_set::id.eq(set_id),
                            sticker_set::title.eq(title),
                            sticker_set::added_by_user_id.eq(added_by_user_id),
                        ))
                        .on_conflict(sticker_set::id)
                        .do_update()
                        .set((
                            sticker_set::title.eq(excluded(sticker_set::title)),
                        ))
                        .execute(conn)?;
                    Ok(())
                })
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn upsert_sticker_set_with_creator(
        &self,
        set_id: &str,
        created_by_user_id: i64,
        added_by_user_id: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let set_id = set_id.to_string();
        self.pool
            .exec(move |conn| {
                conn.immediate_transaction(|conn| {
                    Self::check_removed(&set_id, conn)?;
                    insert_into(sticker_set::table)
                        .values((
                            sticker_set::id.eq(set_id),
                            sticker_set::created_by_user_id.eq(created_by_user_id),
                            sticker_set::added_by_user_id.eq(added_by_user_id),
                        ))
                        .on_conflict(sticker_set::id)
                        .do_update()
                        .set((
                            sticker_set::created_by_user_id.eq(created_by_user_id),
                        ))
                        .execute(conn)?;
                    Ok(())
                })
            })
            .await
    }

    /// title and creator is sometimes not known immediately
    /// does not update last_fetched
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn upsert_sticker_set(
        &self,
        set_id: &str,
        added_by_user_id: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let set_id = set_id.to_string();
        self.pool
            .exec(move |conn| {
                conn.immediate_transaction(|conn| {
                    Self::check_removed(&set_id, conn)?;
                    insert_into(sticker_set::table)
                        .values((
                            sticker_set::id.eq(set_id),
                            sticker_set::added_by_user_id.eq(added_by_user_id),
                        ))
                        .on_conflict(sticker_set::id)
                        .do_nothing()
                        .execute(conn)?;
                    Ok(())
                })
            })
            .await
    }

    fn check_removed(set_id: &str, conn: &mut SqliteConnection) -> Result<(), DatabaseError> {
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
    pub async fn is_sticker_set_banned(
        &self,
        sticker_set_id: &str,
    ) -> Result<bool, DatabaseError> {
        let sticker_set_id = sticker_set_id.to_string();
        self.pool
            .exec(move |conn| {
                let res: i64 = removed_set::table
                    .filter(removed_set::id.eq(sticker_set_id))
                    .select(count_star())
                    .get_result(conn)?;
                Ok(res != 0)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_set_by_sticker_id(
        &self,
        sticker_id: &str,
    ) -> Result<Option<StickerSet>, DatabaseError> {
        let sticker_id = sticker_id.to_string();
        self.pool
            .exec(move |conn| {
                Ok(sticker_set::table
                    .filter(
                        sticker_set::id.eq_any(
                            sticker::table
                                .filter(sticker::id.eq(sticker_id)) // TODO: extract common subqueries to functions returning boxed queries
                                .select((sticker::sticker_set_id)),
                        ),
                    )
                    .select(StickerSet::as_select())
                    .first(conn)
                    .optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn unban_set(&self, set_id: &str) -> Result<Option<i64>, DatabaseError> {
        let set_id = set_id.to_string();
        self.pool
            .exec(move |conn| {
                let original_adder: Option<i64> = delete(removed_set::table.filter(removed_set::id.eq(set_id))).returning(removed_set::added_by_user_id).get_result(conn)?;
                Ok(original_adder)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn ban_set(
        &self,
        set_id: &str,
        added_by_user_id: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let set_id = set_id.to_string();
        self.pool
            .exec(move |conn| {
                insert_into(removed_set::table)
                    .values((
                        removed_set::id.eq(set_id),
                        removed_set::added_by_user_id.eq(added_by_user_id),
                    ))
                    .on_conflict_do_nothing()
                    .execute(conn)?;
                Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn delete_sticker_set(&self, set_id: &str) -> Result<(), DatabaseError> {
        let set_id = set_id.to_string();
        self.pool
            .exec(move |conn| {
                delete(sticker_set::table.filter(sticker_set::id.eq(set_id))).execute(conn)?;
                Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_n_least_recently_fetched_set_ids(
        &self,
        n: i64,
    ) -> Result<Vec<String>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(sticker_set::table
                    .select(sticker_set::id)
                    .order_by(sticker_set::last_fetched)
                    .limit(n)
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    #[deprecated = "use get_latest_sticker_sets instead"]
    pub async fn get_n_latest_sets(&self, n: i64) -> Result<Vec<StickerSet>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                let q = sticker_set::table
                    .select(StickerSet::as_select())
                    .filter(diesel::dsl::exists(
                        sticker::table.filter(sticker::sticker_set_id.eq(sticker_set::id)),
                    ))
                    .order_by(sticker_set::created_at.desc())
                    .limit(n);
                dbg!(debug_query::<Sqlite, _>(&q).to_string());
                Ok(q.load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_latest_stickers(
        &self,
        limit: i64,
        before: NaiveDateTime,
    ) -> Result<Vec<Sticker>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(sticker::table
                    .select(Sticker::as_select())
                    .filter(sticker::created_at.lt(before))
                    .order_by(sticker::created_at.desc())
                    .limit(limit)
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_latest_sticker_sets(
        &self,
        limit: i64,
        before: NaiveDateTime,
    ) -> Result<Vec<StickerSet>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(sticker_set::table
                    .select(StickerSet::as_select())
                    .filter(diesel::dsl::exists(
                        sticker::table.filter(sticker::sticker_set_id.eq(sticker_set::id)),
                    ))
                    .filter(sticker_set::created_at.lt(before))
                    .order_by(sticker_set::created_at.desc())
                    .limit(limit)
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_n_latest_sticker_changes(
        &self,
        n: i64,
    ) -> Result<Vec<StickerChange>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        Ok(sql_query("select sticker.id AS sticker_id, sticker_set_id, count(case when julianday('now') - julianday(sticker_file.created_at) < 1 then true else null end) as today, count(case when julianday('now') - julianday(sticker_file.created_at) < 7 then true else null end) as this_week from sticker inner join sticker_file on sticker.sticker_file_id = sticker_file.id where julianday('now') - julianday(sticker_file.created_at) < 7 group by sticker_set_id order by max(sticker_file.created_at) desc limit 10;")
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_owned_sticker_sets_by_bot(
        &self,
        bot_username: &str,
        user_id: i64,
    ) -> Result<Vec<StickerSet>, DatabaseError> {
        let bot_username = bot_username.to_string();
        self.pool
            .exec(move |conn| {
                Ok(sticker_set::table
                    .filter(sticker_set::created_by_user_id.eq(user_id))
                    .filter(sticker_set::id.like(format!("%_by_{bot_username}")))
                    .select(StickerSet::as_select())
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_owned_sticker_sets(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<StickerSet>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(sticker_set::table
                    .filter(sticker_set::created_by_user_id.eq(user_id))
                    .select(StickerSet::as_select())
                    .order_by(sticker_set::created_at.desc())
                    .limit(limit)
                    .offset(offset)
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_sets(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<StickerSet>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(sticker_set::table
                    .select(StickerSet::as_select())
                    .order_by(sticker_set::id)
                    .limit(limit)
                    .offset(offset)
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_pending_sticker_sets(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<StickerSet>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(sticker_set::table
                    .select(StickerSet::as_select())
                    .filter((sticker_set::is_pending.eq(true)))
                    .order_by(sticker_set::created_at.desc())
                    .limit(limit)
                    .offset(offset)
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_owned_sticker_set_count(&self, user_id: i64) -> Result<i64, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(sticker_set::table
                    .filter(sticker_set::created_by_user_id.eq(user_id))
                    .select(count_star())
                    .first(conn)?)
            })
            .await
    }

    /// returns id of sticker in the set
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn sticker_set_contains_file(
        &self,
        set_id: &str,
        file_id: &str,
    ) -> Result<Option<String>, DatabaseError> {
        let set_id = set_id.to_string();
        let file_id = file_id.to_string();
        self.pool
            .exec(move |conn| {
                Ok(sticker::table
                    .filter(sticker::sticker_set_id.eq(set_id))
                    .filter(sticker::sticker_file_id.eq(file_id))
                    .select((sticker::id))
                    .first(conn)
                    .optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    /// only returns the known set ids
    pub async fn get_set_ids_by_set_ids(
        &self,
        set_ids: &[String],
    ) -> Result<Vec<String>, DatabaseError> {
        let set_ids = set_ids.to_vec();
        self.pool
            .exec(move |conn| {
                Ok(sticker_set::table
                    .filter(sticker_set::id.eq_any(set_ids))
                    .select(sticker_set::id)
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn set_sticker_set_pending(&self, set_id: &str, is_pending: bool) -> Result<(), DatabaseError> {
        let set_id = set_id.to_string();
        self.pool
            .exec(move |conn| {
        let changed = update(sticker_set::table)
            .filter(sticker_set::id.eq(set_id))
            .set((sticker_set::is_pending.eq(is_pending)))
            .execute(conn)?;
        Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_banned_sticker_count_for_set_id(&self, set_id: &str) -> Result<i64, DatabaseError> {
        let set_id = set_id.to_string();
        self.pool.exec(move |conn| Ok(banned_sticker::table.filter(banned_sticker::sticker_set_id.eq(set_id)).count().get_result(conn)?)).await
    }
}
