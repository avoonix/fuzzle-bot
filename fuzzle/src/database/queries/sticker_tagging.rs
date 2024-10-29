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

use crate::database::model::PopularTag;
use crate::database::UserStats;
use crate::util::Emoji;

use super::sticker::max;
use super::DatabaseError;

use super::Database;

use super::super::schema::*;

impl Database {
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn tag_file(
        &self,
        file_id: &str,
        tag_names: &[String],
        user: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let file_id = file_id.to_string();
        let tag_names = tag_names.to_vec();
        self.pool
            .exec(move |conn| {
                conn.immediate_transaction(|conn| {
                    for tag in tag_names {
                        let inserted = insert_into(sticker_file_tag::table)
                            .values((
                                sticker_file_tag::sticker_file_id.eq(&file_id),
                                sticker_file_tag::tag.eq(tag),
                                sticker_file_tag::added_by_user_id.eq(user),
                            ))
                            .on_conflict_do_nothing()
                            .execute(conn)?;
                    }
                    QueryResult::Ok(())
                })?;
                Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn untag_file(
        &self,
        file_id: &str,
        tag_names: &[String],
        user_id: i64,
    ) -> Result<(), DatabaseError> {
        let file_id = file_id.to_string();
        let tag_names = tag_names.to_vec();
        self.pool
            .exec(move |conn| {
                conn.immediate_transaction(|conn| {
                    for tag in &tag_names {
                        // TODO: figure out how to do this in a single query

                        // double optional: the row may not exist or added_by_user_id may be null
                        let Some(added_by_user_id): Option<Option<i64>> = sticker_file_tag::table
                            .filter(sticker_file_tag::tag.eq(tag))
                            .filter(sticker_file_tag::sticker_file_id.eq(&file_id))
                            .select(sticker_file_tag::added_by_user_id)
                            .first(conn)
                            .optional()?
                        else {
                            continue;
                        };

                        let rows_affected = insert_into(sticker_file_tag_history::table)
                            .values((
                                sticker_file_tag_history::sticker_file_id.eq(&file_id),
                                sticker_file_tag_history::tag.eq(tag),
                                sticker_file_tag_history::removed_by_user_id.eq(added_by_user_id),
                                sticker_file_tag_history::added_by_user_id.eq(user_id),
                            ))
                            .execute(conn)?;
                        Self::delete_sticker_file_tag(&file_id, tag, conn)?;
                    }
                    QueryResult::Ok(())
                })?;
                Ok(())
            })
            .await
        // TODO: return a norowsaffected error and make the caller handle it -> in continuous tag mode, you can then inform the user that
        // the tag already existed
    }

    /// except locked stickers
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn tag_all_files_in_set(
        &self,
        set_name: &str,
        tags: &[String],
        user: i64,
    ) -> Result<usize, DatabaseError> {
        let set_name = set_name.to_string();
        let tags = tags.to_vec();
        self.pool
            .exec(move |conn| {
                  let affected =   conn.immediate_transaction(|conn| {
            let mut tags_affected = 0;
        for tag in tags {
            // TODO: translate to proper diesel query?
            tags_affected += sql_query("INSERT INTO sticker_file_tag (sticker_file_id, tag, added_by_user_id)
                                           SELECT DISTINCT sticker_file_id, ?1, ?2 FROM sticker
                                                WHERE sticker.sticker_set_id = ?3 AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL)
                                           ON CONFLICT (sticker_file_id, tag) DO NOTHING")
                                .bind::<Text, _>(tag)
                                .bind::<BigInt, _>(user)
                                .bind::<Text, _>(&set_name)
                                .execute(conn)?;
        }
            QueryResult::Ok(tags_affected)
        })?;
        Ok(affected)
            })
            .await
    }

    /// except locked stickers
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn untag_all_files_in_set(
        &self,
        set_name: &str,
        tags: &[String],
        user: i64,
    ) -> Result<usize, DatabaseError> {
        let set_name = set_name.to_string();
        let tags = tags.to_vec();
        self.pool
            .exec(move |conn| {
                let affected = conn.immediate_transaction(|conn| {
                    let mut tags_affected = 0;
                    for tag in &tags {
                        let result: Vec<(String, Option<i64>)> = sticker_file_tag::table
                            .select((
                                sticker_file_tag::sticker_file_id,
                                sticker_file_tag::added_by_user_id,
                            ))
                            .filter(sticker_file_tag::tag.eq(tag))
                            .filter(
                                sticker_file_tag::sticker_file_id.eq_any(
                                    sticker::table
                                        .select((sticker::sticker_file_id))
                                        .filter(sticker::sticker_set_id.eq(&set_name))
                                        .filter(not(exists(
                                            sticker_file::table
                                                .select(sticker_file::star)
                                                .filter(
                                                    sticker_file::id.eq(sticker::sticker_file_id),
                                                )
                                                .filter(
                                                    sticker_file::tags_locked_by_user_id
                                                        .is_not_null(),
                                                ),
                                        ))),
                                ),
                            )
                            .load(conn)?;

                        for (sticker_file_id, added_by_user_id) in result {
                            tags_affected += insert_into(sticker_file_tag_history::table)
                                .values((
                                    sticker_file_tag_history::sticker_file_id.eq(&sticker_file_id),
                                    sticker_file_tag_history::tag.eq(tag),
                                    sticker_file_tag_history::removed_by_user_id.eq(user),
                                    sticker_file_tag_history::added_by_user_id.eq(added_by_user_id),
                                ))
                                .execute(conn)?;

                            Self::delete_sticker_file_tag(&sticker_file_id, tag, conn)?;
                        }
                    }
                    QueryResult::Ok(tags_affected)
                })?;
                Ok(affected)
            })
            .await
    }

    fn delete_sticker_file_tag(
        sticker_file_id: &str,
        tag: &str,
        // conn: &mut PooledConnection<ConnectionManager<SqliteConnection>>,
        conn: &mut SqliteConnection,
    ) -> Result<usize, diesel::result::Error> {
        delete(
            sticker_file_tag::table
                .filter(sticker_file_tag::sticker_file_id.eq(sticker_file_id))
                .filter(sticker_file_tag::tag.eq(tag)),
        )
        .execute(conn)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_all_sticker_set_tag_counts(
        &self,
        set_id: &str,
    ) -> Result<Vec<(String, i64)>, DatabaseError> {
        let set_id = set_id.to_string();
        self.pool
            .exec(move |conn| {
                Ok(sticker_file_tag::table
                    .group_by(sticker_file_tag::tag)
                    .select((sticker_file_tag::tag, count_star()))
                    .filter(
                        sticker_file_tag::sticker_file_id.eq_any(
                            sticker::table
                                .filter(sticker::sticker_set_id.eq(set_id))
                                .select((sticker::sticker_file_id)),
                        ),
                    )
                    .order_by(count_star().desc())
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_all_sticker_set_tag_counts_by_sticker_file_id(
        &self,
        sticker_file_id: &str,
    ) -> Result<Vec<(String, i64)>, DatabaseError> {
        let sticker_file_id = sticker_file_id.to_string();
        self.pool
            .exec(move |conn| {
                let (sticker1, sticker2) = diesel::alias!(sticker as sticker1, sticker as sticker2);

                Ok(sticker_file_tag::table
                    .group_by(sticker_file_tag::tag)
                    .select((sticker_file_tag::tag, count_star()))
                    .filter(
                        sticker_file_tag::sticker_file_id.eq_any(
                            sticker1
                                .filter(
                                    sticker1.field(sticker::sticker_set_id).eq_any(
                                        sticker2
                                            .filter(
                                                sticker2
                                                    .field(sticker::sticker_file_id)
                                                    .eq(&sticker_file_id),
                                            )
                                            .select(sticker2.field(sticker::sticker_set_id)),
                                    ),
                                )
                                .select((sticker1.field(sticker::sticker_file_id))),
                        ),
                    )
                    .order_by(count_star().desc())
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_all_sticker_set_tag_counts_by_owner_id(
        &self,
        owner_id: i64,
    ) -> Result<Vec<(String, i64)>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(sticker_file_tag::table
                    .group_by(sticker_file_tag::tag)
                    .select((sticker_file_tag::tag, count_star()))
                    .filter(
                        sticker_file_tag::sticker_file_id.eq_any(
                            sticker::table
                                .filter(
                                    sticker::sticker_set_id.eq_any(
                                        sticker_set::table
                                            .filter(sticker_set::created_by_user_id.eq(owner_id))
                                            .select(sticker_set::id),
                                    ),
                                )
                                .select((sticker::sticker_file_id)),
                        ),
                    )
                    .order_by(count_star().desc())
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_popular_tags(&self, limit: i64) -> Result<Vec<PopularTag>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                let tags = sticker_file_tag::table
                    .group_by((sticker_file_tag::tag))
                    .select((sticker_file_tag::tag, count_star()))
                    .order(count_star().desc())
                    .limit(limit)
                    .load(conn)?;

                Ok(tags
                    .into_iter()
                    .map(|(name, count)| PopularTag { name, count })
                    .collect_vec())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_all_tag_emoji_pairs(
        &self,
    ) -> Result<Vec<(Emoji, String, i64)>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                let result: Vec<(String, String, i64)> = sticker_file_tag::table
                    .inner_join(
                        sticker::table
                            .on(sticker::sticker_file_id.eq(sticker_file_tag::sticker_file_id)),
                    )
                    .group_by((
                        // TODO: possible to use regular field selectors?
                        sql::<diesel::sql_types::Text>("sticker.emoji"),
                        sql::<diesel::sql_types::Text>("sticker_file_tag.tag"),
                    ))
                    .select((
                        sql::<diesel::sql_types::Text>("sticker.emoji"),
                        sql::<diesel::sql_types::Text>("sticker_file_tag.tag"),
                        count_star(),
                    ))
                    .load(conn)?;

                Ok(result
                    .into_iter()
                    .map(|(emoji, tag, count)| (Emoji::new_from_string_single(&emoji), tag, count))
                    .collect_vec())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_used_tags(&self) -> Result<Vec<String>, DatabaseError> {
        self.pool
            .exec(move |conn| {
                Ok(sticker_file_tag::table
                    .select(sticker_file_tag::tag)
                    .distinct()
                    .load(conn)?)
            })
            .await
    }
}
