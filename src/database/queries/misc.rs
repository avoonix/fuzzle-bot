use itertools::Itertools;
use sqlx::{Column, Row};

use crate::database::model::Stats;
use crate::database::AdminStats;
use crate::database::SqlDateTime;

use super::DatabaseError;

use super::Database;

impl Database {
    pub async fn run_arbitrary_query(&self, query: String) -> Result<String, DatabaseError> {
        let response = sqlx::query(query.as_str()).fetch_all(&self.pool).await?;

        Ok(format!(
            "{:?}",
            response
                .into_iter()
                .map(|row| row
                    .columns()
                    .iter()
                    .map(|column| format!(
                        "{}: {}",
                        column.name(),
                        row.get::<String, &str>(column.name())
                    ))
                    .collect_vec())
                .collect_vec()
        ))
    }

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

        let least_recently_fetched_set_time: Option<SqlDateTime> = sqlx::query_scalar_unchecked!("select last_fetched from sticker_set order by last_fetched limit 1")
            .fetch_one(&self.pool)
            .await?;

        let now = chrono::Utc::now().naive_utc();
        let stats = AdminStats {
            least_recently_fetched_set_age: least_recently_fetched_set_time.map(|time| now - time),
            number_of_sets_fetched_in_24_hours
        };
        Ok(stats)
    }
}
