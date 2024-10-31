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

use diesel::connection::SimpleConnection;
use flume::Sender;
use futures::future::join_all;
use tracing::{error, log::LevelFilter};

use super::DatabaseError;
use super::User;

use diesel::prelude::*;
use diesel::result::Error;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use flume;

const KB: usize = 1024;
const MB: usize = KB * 1024;
const GB: usize = MB * 1024;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

#[derive(Clone)]
pub struct Database {
    pool: DbWorker,
}

struct Query {
    operation: Box<dyn FnOnce(&mut SqliteConnection) -> Box<dyn Response> + Send>,
    response: Sender<Box<dyn Response>>,
}

trait Response: Send + Any {
    fn unwrap_box(self: Box<Self>) -> Box<dyn Any>;
}

impl<T, E> Response for Result<T, E>
where
    T: Send + 'static,
    E: Send + 'static,
{
    fn unwrap_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

#[derive(Clone)]
pub struct DbWorker {
    sender: Sender<Query>,
}

impl DbWorker {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = flume::unbounded();

        for _ in 0..4 {
            // TODO: add config parameter for number of threads
            // TODO: get rid of unwrap

            let rx = rx.clone();
            let mut conn = SqliteConnection::establish(path.to_str().unwrap())
                .unwrap_or_else(|_| panic!("Error connecting to {}", path.to_str().unwrap()));

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
            std::thread::spawn(move || {
                let mut operations_count = 0;
                while let Ok(Query {
                    operation,
                    response,
                }) = rx.recv()
                {
                    // TODO: log if operation takes more than 100ms
                    let result = operation(&mut conn);
                    let _ = response.send(result);
                    operations_count += 1;
                    if operations_count % 10_000 == 0 {
                        conn.batch_execute("PRAGMA optimize;"); // TODO: log errors
                    }
                }
            });
        }

        DbWorker { sender: tx }
    }

    pub async fn exec<F, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: FnOnce(&mut SqliteConnection) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: From<Error> + Send + 'static,
    {
        let (response_tx, response_rx) = flume::bounded(1);

        self.sender
            .send_async(Query {
                operation: Box::new(move |conn| Box::new(operation(conn)) as Box<dyn Response>),
                response: response_tx,
            })
            .await
            .expect("channel must be open");

        let response = response_rx
            .recv_async()
            .await
            .expect("response channel must be open");

        *Box::<dyn Any>::downcast::<Result<T, E>>(response.unwrap_box()).unwrap()
    }
}

impl Database {
    #[tracing::instrument(name = "Database::new", err(Debug))]
    pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
        let pool: DbWorker = DbWorker::new(path);

        let database = Self { pool };

        database.health_check().await?;
        database.run_migrations().await?;
        database.vacuum().await?;

        Ok(database)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn health_check(&self) -> Result<(), DatabaseError> {
        self.pool
            .exec(|conn| {
                // ensures that the math extensions are activated
                let result = diesel::sql_query("SELECT sin(0)").execute(conn)?;
                Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn vacuum(&self) -> Result<(), DatabaseError> {
        self.pool
            .exec(|conn| {
                let result = diesel::sql_query("VACUUM").execute(conn)?;
                Ok(())
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn run_migrations(&self) -> Result<(), DatabaseError> {
        self.pool
            .exec(|conn| {
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
