use crate::database::model::Stats;
use crate::database::AdminStats;
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
}
