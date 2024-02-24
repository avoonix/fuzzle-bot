mod misc;
mod sticker;
mod tag;
mod user;

use std::path::PathBuf;

use log::error;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use super::DatabaseError;

// https://docs.rs/sqlx/0.5.1/sqlx/sqlite/types/index.html

const KB: usize = 1024;
const MB: usize = KB * 1024;
const GB: usize = MB * 1024;

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::default()
            .max_connections(500)
            .min_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .statement_cache_capacity(500)
                    .pragma("cache_size", format_cache_size(512 * MB))
                    .pragma("temp_store", "memory")
                    .pragma("mmap_size", (2 * GB).to_string())
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                    .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                    .optimize_on_close(true, 1000)
                    .filename(path)
                    .create_if_missing(true),
            )
            .await?;

        if let Err(err) = sqlx::migrate!("./migrations").run(&pool).await {
            error!("error running migrations: {err}");
        }

        sqlx::query!("VACUUM").execute(&pool).await?;

        Ok(Self { pool })
    }
}

fn format_cache_size(bytes: usize) -> String {
    format!("-{}", bytes / 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_size_default() {
        // https://www.sqlite.org/pragma.html#pragma_cache_size
        assert_eq!("-2000", format_cache_size(2000 * KB))
    }
}
