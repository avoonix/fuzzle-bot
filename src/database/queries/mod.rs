mod tag;
mod user;
mod misc;
mod sticker;

use std::path::PathBuf;

use log::error;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use super::DatabaseError;

// https://docs.rs/sqlx/0.5.1/sqlx/sqlite/types/index.html

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::default()
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .create_if_missing(true)
                    .optimize_on_close(true, 1000)
                    .filename(path),
            )
            .await?;

        if let Err(err) = sqlx::migrate!("./migrations").run(&pool).await {
            error!("error running migrations: {err}");
        }

        Ok(Self { pool })
    }
}

#[cfg(test)]
mod tests {}
