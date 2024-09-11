use diesel::dsl::count_distinct;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::BigInt;
use std::collections::HashMap;

use crate::database::model::Stats;
use crate::database::AddedRemoved;
use crate::database::AdminStats;
use crate::database::AggregatedUserStats;
use crate::database::FullUserStats;
use crate::database::PersonalStats;
use crate::database::UserStats;
use crate::database::UserStickerStat;

use super::DatabaseError;

use super::Database;

use super::super::schema::*;

impl Database {
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_personal_stats(&self, user_id: i64) -> Result<PersonalStats, DatabaseError> {
        let conn = &mut self.pool.get()?;
        let favorites: i64 = sticker_user::table
            .filter(sticker_user::is_favorite.eq(true))
            .filter(sticker_user::user_id.eq(user_id))
            .select(count_star())
            .first(conn)?;
        Ok(PersonalStats { favorites })
    }
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_stats(&self) -> Result<Stats, DatabaseError> {
        let conn = &mut self.pool.get()?;
        let sets: i64 = sticker_set::table.select(count_star()).first(conn)?;
        let stickers: i64 = sticker::table.select(count_distinct(sticker::sticker_file_id)).first(conn)?;
        let taggings: i64 = sticker_file_tag::table.select(count_star()).first(conn)?;
        let tagged_stickers: i64 = sticker_file_tag::table
            .select(count_distinct(sticker_file_tag::sticker_file_id))
            .filter(
                diesel::dsl::exists(
                    sticker::table.filter(sticker::sticker_file_id.eq(sticker_file_tag::sticker_file_id))
                )
            )
            .first(conn)?;
        Ok(Stats {
            sets,
            stickers,
            taggings,
            tagged_stickers,
        })
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_admin_stats(&self) -> Result<AdminStats, DatabaseError> {
        let now = chrono::Utc::now().naive_utc(); // TODO: pass time as parameter?
        let conn = &mut self.pool.get()?;
        let number_of_sets_fetched_in_24_hours: i64 = sticker_set::table
            .select(count_star())
            .filter(sticker_set::last_fetched.ge(now - chrono::Duration::hours(24)))
            .first(conn)?;
        let least_recently_fetched_set_time: Option<chrono::NaiveDateTime> = sticker_set::table
            .select(sticker_set::last_fetched) // can't use max since last_fetched may be null in case the set has never been fetched successfully
            .order_by(sticker_set::last_fetched)
            .first(conn)
            .optional()?
            .flatten();
        Ok(AdminStats {
            least_recently_fetched_set_age: least_recently_fetched_set_time.map(|time| now - time),
            number_of_sets_fetched_in_24_hours,
        })
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_user_tagging_stats_24_hours(
        &self,
    ) -> Result<HashMap<Option<i64>, UserStats>, DatabaseError> {
        let now = chrono::Utc::now().naive_utc(); // TODO: pass time as parameter?
        let conn = &mut self.pool.get()?;

        let added_result: Vec<(Option<i64>, i64)> = sticker_file_tag::table
            .group_by((sticker_file_tag::added_by_user_id))
            .select((sticker_file_tag::added_by_user_id, count_star()))
            .filter(sticker_file_tag::created_at.ge(now - chrono::Duration::hours(24)))
            .load(conn)?;
        let removed_result: Vec<(Option<i64>, i64)> = sticker_file_tag_history::table
            .group_by((sticker_file_tag_history::removed_by_user_id))
            .select((sticker_file_tag_history::removed_by_user_id, count_star()))
            .filter(sticker_file_tag_history::created_at.ge(now - chrono::Duration::hours(24)))
            .load(conn)?;

        let mut result: HashMap<Option<i64>, UserStats> = HashMap::new();
        for (user_id, count) in added_result {
            result.entry(user_id).or_default().added_tags = count;
        }
        for (user_id, count) in removed_result {
            result.entry(user_id).or_default().removed_tags = count;
        }

        Ok(result)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn get_removed_tag_counts(
        &self,
        user_id: i64,
        start_date: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<(String, i64)>, DatabaseError> {
        let conn = &mut self.pool.get()?;

        let removed = sticker_file_tag_history::table
            .inner_join(sticker::table.on(sticker::sticker_file_id.eq(sticker_file_tag_history::sticker_file_id)))
            .group_by(sticker::sticker_set_id)
            .select((sticker::sticker_set_id, count_star()))
            .filter(sticker_file_tag_history::removed_by_user_id.eq(user_id));
        let removed = match start_date {
            None => removed.load(conn)?,
            Some(start_date) => removed
                .filter(sticker_file_tag_history::created_at.ge(start_date))
                .load(conn)?,
        };
        Ok(removed)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn get_added_tag_counts(
        &self,
        user_id: i64,
        start_date: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<(String, i64)>, DatabaseError> {
        let conn = &mut self.pool.get()?;

        let added = sticker_file_tag::table
            .inner_join(sticker::table.on(sticker::sticker_file_id.eq(sticker_file_tag::sticker_file_id)))
            .group_by(sticker::sticker_set_id)
            .select((sticker::sticker_set_id, count_star()))
            .filter(sticker_file_tag::added_by_user_id.eq(user_id));
        let added = match start_date {
            None => added.load(conn)?,
            Some(start_date) => added
                .filter(sticker_file_tag::created_at.ge(start_date))
                .load(conn)?,
        };
        Ok(added)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn get_removed_tag_counts_sum(
        &self,
        user_id: i64,
        start_date: Option<chrono::NaiveDateTime>,
    ) -> Result<i64, DatabaseError> {
        let conn = &mut self.pool.get()?;

        let removed = sticker_file_tag_history::table
            .select((count_star()))
            .filter(sticker_file_tag_history::removed_by_user_id.eq(user_id));
        let removed = match start_date {
            None => removed.first(conn)?,
            Some(start_date) => removed
                .filter(sticker_file_tag_history::created_at.ge(start_date))
                .first(conn)?,
        };
        Ok(removed)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn get_added_tag_counts_sum(
        &self,
        user_id: i64,
        start_date: Option<chrono::NaiveDateTime>,
    ) -> Result<i64, DatabaseError> {
        let conn = &mut self.pool.get()?;

        let added = sticker_file_tag::table
            .select((count_star()))
            .filter(sticker_file_tag::added_by_user_id.eq(user_id));
        let added = match start_date {
            None => added.first(conn)?,
            Some(start_date) => added
                .filter(sticker_file_tag::created_at.ge(start_date))
                .first(conn)?,
        };
        Ok(added)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_user_stats(&self, user_id: i64) -> Result<FullUserStats, DatabaseError> {
        let now = chrono::Utc::now().naive_utc();
        let time_24_hours_ago = now - chrono::Duration::hours(24);

        let added_result = self.get_added_tag_counts_sum(user_id, None).await?;
        let added_result_24 = self
            .get_added_tag_counts(user_id, Some(time_24_hours_ago))
            .await?;
        let removed_result = self.get_removed_tag_counts_sum(user_id, None).await?;
        let removed_result_24 = self
            .get_removed_tag_counts(user_id, Some(time_24_hours_ago))
            .await?;

        let mut affected_24: HashMap<String, AddedRemoved> = HashMap::new();

        for (set_id, count) in added_result_24.iter() {
            let entry = affected_24.entry(set_id.to_string()).or_default();
            entry.added += count;
        }

        for (set_id, count) in removed_result_24.iter() {
            let entry = affected_24.entry(set_id.to_string()).or_default();
            entry.removed += count;
        }

        let added_result_24 = added_result_24.into_iter().map(|(_, count)| count).sum();
        let removed_result_24 = removed_result_24.into_iter().map(|(_, count)| count).sum();

        let stats = FullUserStats {
            tagged_24hrs: added_result_24,
            untagged_24hrs: removed_result_24,
            total_tagged: added_result,
            total_untagged: removed_result,
            sets: affected_24,
        };
        Ok(stats)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_general_user_stats(&self, limit: i64, offset: i64) -> Result<Vec<UserStickerStat>, DatabaseError> {
        Ok(sql_query("select sticker_set.created_by_user_id as user_id, count(*) as set_count, username.tg_username as username from sticker_set left join username on username.tg_id = sticker_set.created_by_user_id where user_id is not null group by created_by_user_id order by set_count desc limit ?1 offset ?2;")
                                .bind::<BigInt, _>(limit)
                                .bind::<BigInt, _>(offset)
            .load(&mut self.pool.get()?)?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_aggregated_user_stats(&self) -> Result<AggregatedUserStats, DatabaseError> {
        let unique_sticker_owners = sticker_set::table
            .select(count_distinct(sticker_set::created_by_user_id))
            .first(&mut self.pool.get()?)?;
        Ok(AggregatedUserStats {
            unique_sticker_owners,
        })
    }
}
