mod export;
mod moderation_task;
mod stats;
mod sticker;
mod sticker_set;
mod sticker_tagging;
mod tag;
mod user;
mod username;

use std::any::Any;
use std::{path::PathBuf, time::Duration};

use deadpool_diesel::Runtime;
use deadpool_diesel::sqlite::{Hook, HookError, Manager, Pool};
use diesel::connection::SimpleConnection;
use futures::channel::mpsc::channel;
use futures::future::join_all;
use tokio::sync::mpsc::{Sender, UnboundedSender, unbounded_channel};
use tracing::{error, log::LevelFilter};

use crate::bot::InternalError;

use super::DatabaseError;
use super::User;

use diesel::prelude::*;
use diesel::result::Error;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

const KB: usize = 1024;
const MB: usize = KB * 1024;
const GB: usize = MB * 1024;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub type SqlitePool = deadpool_diesel::Pool<deadpool_diesel::Manager<diesel::SqliteConnection>>;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    #[tracing::instrument(name = "Database::new", err(Debug))]
    pub async fn new(path: PathBuf) -> Result::<Self, InternalError> {
        let manager = Manager::new(path.to_str().unwrap(), Runtime::Tokio1);
         let pool = Pool::builder(manager)
        .max_size(69)
        .pre_recycle(Hook::async_fn(|obj, metrics| {
            Box::pin(async move {
                if metrics.recycle_count % 1_000 == 0 {
                    let res = obj.interact(|conn| conn.batch_execute("PRAGMA optimize;")).await;
                match res {
                    Ok(_) => Ok(()),
                    // hopeless wrangling with error types here, idk how to get the diesel error
                    // into a HookError
                    Err(_err) => {
                        print!("sqlite error!!!");
                        Err(HookError::message("error configuring database connection"))
                    }
                }
                } else {
                    Ok(())
                }
            })
        })) // TODO: add a hook that calls optimize before the connection is recycled
        .recycle_timeout(Some(Duration::from_hours(1)))
        .post_create(Hook::async_fn(|obj, _| {
            Box::pin(async move {
                let res = obj
                    .interact(|conn| {
                       
            conn.batch_execute(&format!(
                "PRAGMA busy_timeout = {};",
                Duration::from_secs(60).as_millis()
            ))
            .unwrap();
            conn.batch_execute("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA wal_autocheckpoint = 1000; PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
            conn.batch_execute("PRAGMA foreign_keys = ON; PRAGMA temp_store = MEMORY;")
                .unwrap();
            conn.batch_execute(&format!(
                "PRAGMA cache_size = {};",
                format_cache_size(2 * GB)
            ))
            .unwrap();
            conn.batch_execute(&format!("PRAGMA mmap_size = {};", (4 * GB).to_string()))
                .unwrap();
            conn.batch_execute("PRAGMA optimize=0x10002;").unwrap();
                    })
                    .await;
                match res {
                    Ok(_) => Ok(()),
                    // hopeless wrangling with error types here, idk how to get the diesel error
                    // into a HookError
                    Err(_err) => {
                        print!("sqlite error!!!");
                        Err(HookError::message("error configuring database connection"))
                    }
                }
            })
        }))
        .runtime(Runtime::Tokio1)
        .build()
        .expect("pool to be valid");

        let database = Self { pool };

        database.health_check().await?;
        database.run_migrations().await?;
        database.vacuum().await?;

        Ok(database)
    }

    async fn exec<F, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: FnOnce(&mut SqliteConnection) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: From<deadpool::managed::PoolError<deadpool_diesel::Error>> 
            + From<deadpool_diesel::InteractError> 
            + Send + 'static,
    {
        self.pool.get().await?.interact(operation).await?
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn health_check(&self) -> Result<(), DatabaseError> {
        self.exec(|conn| {
                // ensures that the math extensions are activated
                let result = diesel::sql_query("SELECT sin(0)").execute(conn)?;
                Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn vacuum(&self) -> Result<(), DatabaseError> {
        self.exec(|conn| {
                let result = diesel::sql_query("VACUUM").execute(conn)?;
                Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn run_migrations(&self) -> Result<(), DatabaseError> {
        self.exec(|conn| {
                conn.run_pending_migrations(MIGRATIONS)
                    .map_err(|err| anyhow::anyhow!(err.to_string()))?;

                Ok(())
            })
            .await
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
