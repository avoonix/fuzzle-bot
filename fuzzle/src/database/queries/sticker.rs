use base64::{engine::general_purpose, Engine};
use diesel::{
    delete, dsl::{count_star, now, sql}, insert_into, prelude::*, sql_query, sql_types::{BigInt, Nullable, Text}, update, upsert::excluded
};
use itertools::Itertools;
use tracing::warn;

use crate::{
    database::{
        BanReason, BannedSticker, MergeStatus, Order, Sticker, StickerFile, StickerIdStickerFileId, StickerSet, StickerType, StickerUser, min_max, query_builder::StickerTagQuery
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

define_sql_function! {
    /// Represents the `random()` function
    fn random() -> diesel::sql_types::Integer;
}

impl Database {
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn update_last_fetched(&self, set_id: String) -> Result<(), DatabaseError> {
        self.pool
            .exec(move |conn| {
        let changed = update(sticker_set::table)
            .filter(sticker_set::id.eq(set_id))
            .set((sticker_set::last_fetched.eq(now)))
            .execute(conn)?;
        Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_set_by_id(
        &self,
        sticker_set_id: &str,
    ) -> Result<Option<StickerSet>, DatabaseError> {
        let sticker_set_id = sticker_set_id.to_string();
        self.pool
            .exec(move |conn| {
        let set = sticker_set::table
            .filter(sticker_set::id.eq(sticker_set_id))
            .select(StickerSet::as_select())
            .first(conn)
            .optional()?;
        Ok(set)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn update_sticker(
        &self,
        sticker_id: String,
        file_id: String,
    ) -> Result<(), DatabaseError> {
        self.pool
            .exec(move |conn| {
        let changed = update(sticker::table)
            .filter(sticker::id.eq(sticker_id))
            .set((sticker::telegram_file_identifier.eq(file_id)))
            .execute(conn)?;
        Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn create_file(
        &self,
        sticker_file_id: &str,
        thumbnail_file_id: Option<String>,
        sticker_type: StickerType,
    ) -> Result<(), DatabaseError> {
        let sticker_file_id = sticker_file_id.to_string();
        self.pool
            .exec(move |conn| {
                conn.immediate_transaction(|conn| {
                    Self::check_removed_sticker(&sticker_file_id, conn)?;
        let q = insert_into(sticker_file::table)
            .values((
                sticker_file::id.eq(sticker_file_id),
                sticker_file::thumbnail_file_id.eq(thumbnail_file_id.clone()),
                sticker_file::sticker_type.eq(sticker_type),
            ))
            .on_conflict(sticker_file::id);
        if let Some(thumbnail_file_id) = thumbnail_file_id {
            q.do_update()
                .set(sticker_file::thumbnail_file_id.eq(thumbnail_file_id))
                .execute(conn)?;
        } else {
            q.do_nothing().execute(conn)?;
        }
        Ok(())
                })
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn find_canonical_sticker_file_id(&self, sticker_file_id: &str) -> Result<String, DatabaseError> {
        let sticker_file_id = sticker_file_id.to_string();
        self.pool
            .exec(move |conn| {
        let mut sticker_file_id = sticker_file_id.to_string();
        let mut iterations = 0;
        loop {
            let merged_sticker_file_id = merged_sticker::table
                .select(merged_sticker::canonical_sticker_file_id)
                .filter(merged_sticker::removed_sticker_file_id.eq(&sticker_file_id))
                .first(conn)
                .optional()?;
            iterations += 1;
            if iterations > 100 {
                tracing::error!("potential infinite loop for file {sticker_file_id}");
                break;
            }
            if let Some(merged_sticker_file_id) = merged_sticker_file_id {
                sticker_file_id = merged_sticker_file_id;
            } else {
                break;
            }
        }
        Ok(sticker_file_id)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn create_sticker(
        &self,
        sticker_id: &str,
        file_id: &str,
        emoji: Option<Emoji>,
        set_id: &str,
        sticker_file_id: &str,
    ) -> Result<(), DatabaseError> {
        let sticker_id = sticker_id.to_string();
        let file_id = file_id.to_string();
        let set_id = set_id.to_string();
        let sticker_file_id = sticker_file_id.to_string();
        self.pool
            .exec(move |conn| {
        insert_into(sticker::table)
            .values((
                sticker::id.eq(sticker_id),
                sticker::sticker_set_id.eq(set_id),
                sticker::telegram_file_identifier.eq(file_id),
                sticker::sticker_file_id.eq(sticker_file_id),
                sticker::emoji.eq(emoji.map(|e| e.to_string_without_variant())),
            ))
            .on_conflict_do_nothing()
            .execute(conn)?;

        Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_all_stickers_in_set(
        &self,
        set_id: &str,
    ) -> Result<Vec<Sticker>, DatabaseError> {
        let set_id = set_id.to_string();
        self.pool
            .exec(move |conn| {
        Ok(sticker::table
            .filter(sticker::sticker_set_id.eq(set_id))
            .select(Sticker::as_select())
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_by_id(
        &self,
        sticker_id: &str,
    ) -> Result<Option<Sticker>, DatabaseError> {
        let sticker_id = sticker_id.to_string();
        self.pool
            .exec(move |conn| {
        Ok(sticker::table
            .filter(sticker::id.eq(sticker_id))
            .select(Sticker::as_select())
            .first(conn)
            .optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_some_sticker_by_file_id(
        &self,
        sticker_file_id: &str,
    ) -> Result<Option<Sticker>, DatabaseError> {
        let sticker_file_id = sticker_file_id.to_string();
        self.pool
            .exec(move |conn| {
        Ok(sticker::table
            .filter(sticker::sticker_file_id.eq(sticker_file_id))
            .select(Sticker::as_select())
            .first(conn)
            .optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_tags_by_file_id(&self, sticker_file_id: &str) -> Result<Vec<String>, DatabaseError> {
        let sticker_file_id = sticker_file_id.to_string();
        self.pool
            .exec(move |conn| {
        Ok(sticker_file_tag::table
            .filter(sticker_file_tag::sticker_file_id.eq(sticker_file_id))
            .select((sticker_file_tag::tag))
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    #[deprecated(note = "use get_sticker_tags_by_file_id instead")]
    pub async fn get_sticker_tags(&self, sticker_id: &str) -> Result<Vec<String>, DatabaseError> {
        let sticker_id = sticker_id.to_string();
        self.pool
            .exec(move |conn| {
        // TODO: pass file_id instead
        Ok(sticker_file_tag::table
            .filter(
                sticker_file_tag::sticker_file_id.eq_any(
                    sticker::table
                        .filter(sticker::id.eq(sticker_id))
                        .select((sticker::sticker_file_id)), // .single_value()
                ),
            )
            .select((sticker_file_tag::tag))
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_multiple_sticker_tags(
        &self,
        sticker_ids: Vec<String>,
    ) -> Result<Vec<(String, i64)>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        Ok(sticker_file_tag::table
            .group_by((sticker_file_tag::tag))
            .filter(
                sticker_file_tag::sticker_file_id.eq_any(
                    sticker::table
                        .filter(sticker::id.eq_any(sticker_ids))
                        .select((sticker::sticker_file_id)), // .single_value()
                ),
            )
            .select((sticker_file_tag::tag, count_star()))
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_stickers_for_tag_query(
        &self,
        tags: Vec<String>, // tags are anded (solo AND mammal)
        blacklist: Vec<String>,
        emoji: Vec<String>, // emojis are ored (<smile emoji> OR <paw emoji>)
        limit: i64,
        offset: i64,
        order: Order,
    ) -> Result<Vec<Sticker>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        let query = StickerTagQuery::new(tags, blacklist)
            .emoji(emoji)
            .limit(limit)
            .offset(offset)
            .order(order);

        let stickers: Vec<Sticker> = query.generate().load(conn)?;
        Ok(stickers)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_sets_for_tag_query(
        &self,
        tags: Vec<String>, // tags are anded (solo AND mammal)
        blacklist: Vec<String>,
        // emoji: Vec<String>, // emojis are ored (<smile emoji> OR <paw emoji>)
        limit: i64,
        offset: i64,
        // order: Order,
    ) -> Result<Vec<StickerSet>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        let query = StickerTagQuery::new(tags, blacklist)
            // .emoji(emoji)
            .limit(limit)
            .offset(offset)
            .sets();
            // .order(order);

        let sets: Vec<StickerSet> = query.generate().load(conn)?;
        Ok(sets)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_random_sticker_to_tag(&self) -> Result<Option<Sticker>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        Ok(sticker::table
            .filter(
                sticker::sticker_file_id.ne_all(sticker_file_tag::table.select((sticker_file_tag::sticker_file_id))),
            )
            .select(Sticker::as_select())
            .order(random())
            .first(conn)
            .optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_random_sticker(&self) -> Result<Option<Sticker>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        Ok(sticker::table
            .select(Sticker::as_select())
            .order(random())
            .first(conn)
            .optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_files_by_ids(&self, sticker_file_ids: &[String]) -> Result<Vec<StickerFile>, DatabaseError> {
        let sticker_file_ids = sticker_file_ids.to_vec();
        self.pool
            .exec(move |conn| {
        Ok(sticker_file::table
            .filter(
                sticker_file::id.eq_any( sticker_file_ids),
            )
            .select(StickerFile::as_select())
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_file_by_sticker_id(&self, sticker_id: &str) -> Result<Option<StickerFile>, DatabaseError> {
        let sticker_id = sticker_id.to_string();
        self.pool
            .exec(move |conn| {
        Ok(sticker_file::table
            .filter(
                sticker_file::id.eq_any(
                    sticker::table
                        .filter((sticker::id.eq(sticker_id)))
                        .select((sticker::sticker_file_id)),
                ),
            )
            .select(StickerFile::as_select())
            .first(conn)
            .optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_some_sticker_ids_for_sticker_file_ids(
        &self,
        sticker_file_ids: Vec<String>,
    ) -> Result<Vec<StickerIdStickerFileId>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        let result: Vec<(String, String)> = sticker::table
            .group_by((sticker::sticker_file_id))
            .select((sticker::sticker_file_id, max(sticker::id)))
            .filter(sticker::sticker_file_id.eq_any(sticker_file_ids))
            .load(conn)?;
        Ok(result
            .into_iter()
            .map(|(sticker_file_id, sticker_id)| StickerIdStickerFileId {
                sticker_file_id: sticker_file_id,
                sticker_id,
            })
            .collect_vec())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_file_ids_by_sticker_id(
        &self,
        sticker_ids: &[String],
    ) -> Result<Vec<String>, DatabaseError> {
        let sticker_ids = sticker_ids.to_vec();
        self.pool
            .exec(move |conn| {
        let result: Vec<String> = sticker::table
            .select(sticker::sticker_file_id)
            .distinct()
            .filter(sticker::id.eq_any(sticker_ids))
            .load(conn)?;
        Ok(result)
            })
            .await
    }

    /// including itself (so there's always at least one entry in the list)
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_overlapping_sets(
        &self,
        set_id: &str,
    ) -> Result<Vec<(String, i64)>, DatabaseError> {
        let set_id = set_id.to_string();
        self.pool
            .exec(move |conn| {
        let (sticker1, sticker2) = diesel::alias!(sticker as sticker1, sticker as sticker2);
        Ok(sticker1
            .group_by(sticker1.field(sticker::sticker_set_id))
            // .filter(sticker1.field(sticker::sticker_set_id).ne(&set_id))
            .filter(
                sticker1.field(sticker::sticker_file_id).eq_any(
                    sticker2
                        .filter(sticker2.field(sticker::sticker_set_id).eq(&set_id))
                        .select(sticker2.field(sticker::sticker_file_id)),
                ),
            )
            .select((sticker1.field(sticker::sticker_set_id), count_star()))
            .order_by(count_star().desc())
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sets_containing_file(
        &self,
        sticker_file_id: &str,
    ) -> Result<Vec<StickerSet>, DatabaseError> {
        let sticker_file_id = sticker_file_id.to_string();
        self.pool
            .exec(move |conn| {
        Ok(sticker_set::table
            .filter(
                sticker_set::id.eq_any(
                    sticker::table
                        .filter(sticker::sticker_file_id.eq(sticker_file_id))
                        .select(sticker::sticker_set_id),
                ),
            )
            .select(StickerSet::as_select())
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn update_file_lock(
        &self,
        file_id: &str,
        user_id: i64,
        locked: bool,
    ) -> Result<(), DatabaseError> {
        let file_id = file_id.to_string();
        self.pool
            .exec(move |conn| {
        let user_id = if locked { Some(user_id) } else { None };
        let updated_rows = diesel::update(sticker_file::table)
            .filter(sticker_file::id.eq(file_id))
            .set(sticker_file::tags_locked_by_user_id.eq(user_id))
            .execute(conn)?;
        #[cfg(debug_assertions)]
        assert_eq!(updated_rows, 1);
        Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_emojis(&self, sticker_id: &str) -> Result<Vec<Emoji>, DatabaseError> {
        let sticker_id = sticker_id.to_string();
        self.pool
            .exec(move |conn| {
        let (sticker1, sticker2) = diesel::alias!(sticker as sticker1, sticker as sticker2);
        let emojis: Vec<Option<String>> = sticker1
            .filter(
                sticker1.field(sticker::sticker_file_id).eq_any(
                    sticker2
                        .filter(sticker2.field(sticker::id).eq(sticker_id))
                        .select(sticker2.field(sticker::sticker_file_id)),
                ),
            )
            .select(sticker1.field(sticker::emoji))
            .load(conn)?;

        Ok(emojis
            .into_iter()
            .filter_map(|e| e.map(|e| Emoji::new_from_string_single(&e)))
            .collect_vec())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn delete_sticker(&self, sticker_id: &str) -> Result<(), DatabaseError> {
        let sticker_id = sticker_id.to_string();
        self.pool
            .exec(move |conn| {
        delete(sticker::table.filter(sticker::id.eq(sticker_id))).execute(conn)?;
        Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn clean_up_sticker_files_without_stickers_and_without_tags(&self) -> Result<Vec<String>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        let current_time = chrono::Utc::now().naive_utc(); // TODO: pass time as parameter?
        Ok(delete(
            sticker_file::table
                .filter(sticker_file::id.ne_all(sticker::table.select(sticker::sticker_file_id)))
                .filter(sticker_file::id.ne_all(sticker_file_tag::table.select(sticker_file_tag::sticker_file_id)))
                .filter(sticker_file::created_at.lt(current_time - chrono::Duration::hours(24)))
        ).returning(sticker_file::id) 
        .load(conn)?)
            })
            .await
    }

// select * from sticker_file where id not in (select sticker_file_id from sticker) and id not in (select sticker_file_id from sticker_file_tag) and created_at < '2024-10-22 05:47:51';


    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_sticker_user(
        &self,
        sticker_id: &str,
        user_id: i64,
    ) -> Result<Option<StickerUser>, DatabaseError> {
        let sticker_id = sticker_id.to_string();
        self.pool
            .exec(move |conn| {
        // TODO: sort by favorites (when favoriting is implemented)
        Ok(sticker_user::table
            .filter(sticker_user::user_id.eq(user_id))
            .filter(sticker_user::sticker_id.eq(sticker_id))
            .select(StickerUser::as_select())
            .first(conn).optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_recently_used_stickers(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Sticker>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        Ok(sticker::table
            .inner_join(sticker_user::table)
            .filter(sticker_user::user_id.eq(user_id))
            .select(Sticker::as_select())
            .order_by((sticker_user::is_favorite.desc(), sticker_user::last_used.desc()))
            .limit(limit)
            .offset(offset)
            .load(conn)?)
            })
            .await
    }

    /// returns 2 file ids
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_random_potential_merge_file_ids(
        &self,
    ) -> Result<Option<(String, String)>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        Ok(potentially_similar_file::table
            .select((potentially_similar_file::file_id_a, potentially_similar_file::file_id_b))
            .filter(potentially_similar_file::status.eq(MergeStatus::Queued))
            .order_by(random())
            .limit(1)
            .first(conn)
            .optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn add_or_modify_potential_merge(
        &self,
        file_id_a: &str,
        file_id_b: &str,
        status: MergeStatus,
    ) -> Result<(), DatabaseError> {
        let file_id_a = file_id_a.to_string();
        let file_id_b = file_id_b.to_string();
        self.pool
            .exec(move |conn| {
        let (smaller, bigger) = min_max(file_id_a, file_id_b);
        insert_into(potentially_similar_file::table)
            .values((potentially_similar_file::file_id_a.eq(smaller), potentially_similar_file::file_id_b.eq(bigger), potentially_similar_file::status.eq(status)))
            .on_conflict((potentially_similar_file::file_id_a, potentially_similar_file::file_id_b))
            .do_update()
            .set(potentially_similar_file::status.eq(status))
            .execute(conn)?;
        Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_all_merge_candidate_file_ids(
        &self,
        file_id: &str,
    ) -> Result<Vec<String>, DatabaseError> {
        let file_id = file_id.to_string();
        self.pool
            .exec(move |conn| {
        Ok(potentially_similar_file::table
            .select(potentially_similar_file::file_id_a)
            .distinct()
            .filter(potentially_similar_file::file_id_b.eq(&file_id))
            .union(potentially_similar_file::table
                .select(potentially_similar_file::file_id_b)
                .distinct()
                .filter(potentially_similar_file::file_id_a.eq(&file_id)))
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn merge_stickers(
        &self,
        canonical_file_id: &str,
        duplicate_file_id: &str,
        user_id: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let canonical_file_id = canonical_file_id.to_string();
        let duplicate_file_id = duplicate_file_id.to_string();
              self.pool
            .exec(move |conn| { Ok( 
                    conn.immediate_transaction(|conn| {
            let stickers_affected_merge = sql_query("INSERT INTO merged_sticker (canonical_sticker_file_id, removed_sticker_file_id, removed_sticker_id, removed_sticker_set_id, created_by_user_id)
               SELECT ?1, sticker_file_id, id, sticker_set_id, ?2 FROM sticker WHERE sticker_file_id = ?3")
                                .bind::<Text, _>(&canonical_file_id)
                                .bind::<Nullable<BigInt>, _>(user_id)
                                .bind::<Text, _>(&duplicate_file_id)
                                .execute(conn)?;
                            
            sql_query("INSERT INTO sticker_file_tag (sticker_file_id, tag, added_by_user_id) SELECT ?1, tag, added_by_user_id FROM sticker_file_tag WHERE sticker_file_id = ?2
                     ON CONFLICT(sticker_file_id, tag) DO NOTHING")
                                .bind::<Text, _>(&canonical_file_id)
                                .bind::<Text, _>(&duplicate_file_id)
                                .execute(conn)?;
            
            let stickers_affected_update = update(sticker::table)
                .filter(sticker::sticker_file_id.eq(&duplicate_file_id))
                .set(sticker::sticker_file_id.eq(&canonical_file_id))
                                .execute(conn)?;

            delete(
                sticker_file_tag::table.filter(sticker_file_tag::sticker_file_id.eq(&duplicate_file_id))
            ).execute(conn)?;
            
            delete(
                sticker_file::table.filter(sticker_file::id.eq(&duplicate_file_id))
            ).execute(conn)?;

            #[cfg(debug_assertions)]
            assert!(stickers_affected_merge == stickers_affected_update);

            QueryResult::Ok(())
        })?)

            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_most_duplicated_stickers(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Sticker>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        Ok(sql_query("SELECT * FROM sticker GROUP BY sticker.sticker_file_id ORDER BY count(*) DESC LIMIT ?1 OFFSET ?2")
                                .bind::<BigInt, _>(limit)
                                .bind::<BigInt, _>(offset)
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_stickers_by_emoji(
        &self,
        emoji: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Sticker>, DatabaseError> {
        let emoji = emoji.to_string();
        self.pool
            .exec(move |conn| {
        // TODO: does not support random sort
        Ok(sql_query("SELECT * FROM sticker WHERE emoji = ?1 GROUP BY sticker.sticker_file_id LIMIT ?2 OFFSET ?3")
                                .bind::<Text, _>(emoji)
                                .bind::<BigInt, _>(limit)
                                .bind::<BigInt, _>(offset)
            .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_most_used_emojis(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(Emoji, i64)>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        let result: Vec<(Option<String>, i64)> = sticker::table
            .group_by((sticker::emoji))
            .select((sticker::emoji, count_star()))
            .order_by(count_star().desc())
            .limit(limit)
            .offset(offset)
            .load(conn)?;

        Ok(result.into_iter().filter_map(|res| res.0.map(|emoji| (Emoji::new_from_string_single(emoji), res.1))).collect_vec())
            })
            .await
    }

    /// returns the set id of the now unbanned sticker; error if it was not banned
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn unban_sticker(&self, sticker_id: &str) -> Result<(String, String), DatabaseError> {
        let sticker_id = sticker_id.to_string();
        self.pool
            .exec(move |conn| {
                let res = delete(banned_sticker::table.filter(banned_sticker::id.eq(sticker_id))).returning((banned_sticker::sticker_set_id, banned_sticker::sticker_file_id)).get_result(conn)?;
                Ok(res)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn ban_sticker(
        &self,
        sticker_id: &str,
        telegram_file_identifier: &str,
        sticker_set_id: &str,
        sticker_file_id: &str,
        thumbnail_file_id: &Option<String>,
        sticker_type: StickerType,
        clip_max_match_distance: f32,
        ban_reason: BanReason,
    ) -> Result<(), DatabaseError> {
        let sticker_id = sticker_id.to_string();
        let telegram_file_identifier = telegram_file_identifier.to_string();
        let sticker_set_id = sticker_set_id.to_string();
        let sticker_file_id = sticker_file_id.to_string();
        let thumbnail_file_id = thumbnail_file_id.clone();
        self.pool
            .exec(move |conn| {
                insert_into(banned_sticker::table)
                    .values((
                        banned_sticker::id.eq(sticker_id),
                        banned_sticker::telegram_file_identifier.eq(telegram_file_identifier),
                        banned_sticker::sticker_set_id.eq(sticker_set_id),
                        banned_sticker::sticker_file_id.eq(sticker_file_id),
                        banned_sticker::thumbnail_file_id.eq(thumbnail_file_id),
                        banned_sticker::sticker_type.eq(sticker_type),
                        banned_sticker::clip_max_match_distance.eq(clip_max_match_distance),
                        banned_sticker::ban_reason.eq(ban_reason),
                    ))
                    .on_conflict_do_nothing()
                    .execute(conn)?;
                Ok(())
            })
            .await
    }

    fn check_removed_sticker(sticker_file_id: &str, conn: &mut SqliteConnection) -> Result<(), DatabaseError> {
        let removed: Option<String> = banned_sticker::table
            .filter(banned_sticker::sticker_file_id.eq(sticker_file_id))
            .select((banned_sticker::id))
            .first(conn)
            .optional()?;
        if removed.is_some() {
            Err(DatabaseError::TryingToInsertRemovedSticker)
        } else {
            Ok(())
        }
    }
    
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_banned_sticker_max_match_distance(
        &self,
        file_id: &str
    ) -> Result<Option<f32>, DatabaseError> {
        let file_id = file_id.to_string();
        self.pool
            .exec(move |conn| {
        let result = banned_sticker::table
            .filter((banned_sticker::sticker_file_id.eq(file_id)))
            .select((banned_sticker::clip_max_match_distance))
            .first(conn).optional()?;
                Ok(result)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_banned_sticker(
        &self,
        sticker_id: &str,
    ) -> Result<Option<BannedSticker>, DatabaseError> {
        let sticker_id = sticker_id.to_string();
        self.pool
            .exec(move |conn| {
        Ok(banned_sticker::table
            .filter(banned_sticker::id.eq(sticker_id))
            .select(BannedSticker::as_select())
            .first(conn).optional()?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_banned_stickers(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<BannedSticker>, DatabaseError> {
        self.pool
            .exec(move |conn| {
        Ok(banned_sticker::table
            .order_by(banned_sticker::created_at.desc())
            .limit(limit)
            .offset(offset)
            .select(BannedSticker::as_select())
            .load(conn)?)
            })
            .await
    }
}
