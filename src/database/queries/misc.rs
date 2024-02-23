use std::collections::HashMap;

use crate::database::model::Stats;
use crate::database::AddedRemoved;
use crate::database::AdminStats;
use crate::database::FullUserStats;
use crate::database::SqlDateTime;

use super::DatabaseError;

use super::Database;

impl Database {
    pub async fn get_stats(&self) -> Result<Stats, DatabaseError> {
        let number_of_sets: i64 = sqlx::query_scalar_unchecked!("SELECT COUNT(*) FROM sticker_set")
            .fetch_one(&self.pool)
            .await?;

        let number_of_stickers: i64 = sqlx::query_scalar_unchecked!("SELECT COUNT(*) FROM sticker")
            .fetch_one(&self.pool)
            .await?;

        let number_of_taggings: i64 =
            sqlx::query_scalar_unchecked!("SELECT COUNT(*) FROM file_hash_tag")
                .fetch_one(&self.pool)
                .await?;

        let stats = Stats {
            sets: number_of_sets,
            stickers: number_of_stickers,
            taggings: number_of_taggings,
        };
        Ok(stats)
    }

    pub async fn get_admin_stats(&self) -> Result<AdminStats, DatabaseError> {
        let number_of_sets_fetched_in_24_hours: i64 = sqlx::query_scalar_unchecked!("select count(*) from sticker_set where julianday('now') - julianday(last_fetched) <= 1")
            .fetch_one(&self.pool)
            .await?;

        // might not exist (because no sets) or might be null (because error during fetch)
        let least_recently_fetched_set_time: Option<Option<SqlDateTime>> =
            sqlx::query_scalar_unchecked!(
                "select last_fetched from sticker_set order by last_fetched limit 1"
            )
            .fetch_optional(&self.pool)
            .await?;

        let now = chrono::Utc::now().naive_utc();
        let stats = AdminStats {
            least_recently_fetched_set_age: least_recently_fetched_set_time
                .flatten()
                .map(|time| now - time),
            number_of_sets_fetched_in_24_hours,
        };
        Ok(stats)
    }

    pub async fn get_user_stats(&self, user_id: u64) -> Result<FullUserStats, DatabaseError> {
        let user_id = user_id as i64;

        let added_result_24 = sqlx::query!("SELECT count(*) AS \"count: i64\" FROM file_hash_tag WHERE added_by_user_id = ?1 AND julianday('now') - julianday(created_at) <= 1", user_id)
            .fetch_one(&self.pool)
            .await?;
        let removed_result_24 = sqlx::query!("SELECT count(*) AS \"count: i64\" FROM file_hash_tag_history WHERE removed_by_user_id = ?1 AND julianday('now') - julianday(created_at) <= 1", user_id)
            .fetch_one(&self.pool)
            .await?;

        let added_result = sqlx::query!(
            "SELECT count(*) AS \"count: i64\" FROM file_hash_tag WHERE added_by_user_id = ?1",
            user_id
        )
        .fetch_one(&self.pool)
        .await?;
        let removed_result = sqlx::query!("SELECT count(*) AS \"count: i64\" FROM file_hash_tag_history WHERE removed_by_user_id = ?1", user_id)
            .fetch_one(&self.pool)
            .await?;

        let interactions = sqlx::query!("SELECT interactions FROM user WHERE id = ?1", user_id)
            .fetch_one(&self.pool)
            .await?;

        let affected_sets_24 = sqlx::query!("select 'tagged' as operation, set_id, count() as count from file_hash_tag left join sticker on sticker.file_hash = file_hash_tag.file_hash where file_hash_tag.added_by_user_id = ?1 AND julianday('now') - julianday(file_hash_tag.created_at) <= 1 group by sticker.file_hash
UNION
select 'untagged' as operation, set_id, count() as count from file_hash_tag_history left join sticker on sticker.file_hash = file_hash_tag_history.file_hash where file_hash_tag_history.removed_by_user_id = ?1 AND julianday('now') - julianday(file_hash_tag_history.created_at) <= 1 group by sticker.file_hash;", user_id)
            .fetch_all(&self.pool)
            .await?;

        let mut affected_24 = HashMap::new();

        for affected in affected_sets_24 {
            // set id might be missing if the set was banned
            if let Some(set_id) = affected.set_id {
                let entry = affected_24.entry(set_id).or_insert(AddedRemoved {
                    added: 0,
                    removed: 0,
                });
                match affected.operation.as_ref() {
                    "tagged" => entry.added += affected.count,
                    "untagged" => entry.removed += affected.count,
                    _ => Err(anyhow::anyhow!("invalid operation {}", affected.operation))?,
                }
            }
        }

        let stats = FullUserStats {
            interactions: interactions.interactions,
            tagged_24hrs: added_result_24.count,
            untagged_24hrs: removed_result_24.count,
            total_tagged: added_result.count,
            total_untagged: removed_result.count,
            sets: affected_24,
        };
        Ok(stats)
    }
}
