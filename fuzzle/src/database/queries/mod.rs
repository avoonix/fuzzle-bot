mod stats;
mod sticker;
mod sticker_set;
mod tag;
mod user;
mod export;

use std::{path::PathBuf, time::Duration};

use diesel::connection::SimpleConnection;
use tracing::{error, log::LevelFilter};

use super::DatabaseError;

use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::result::Error;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

const KB: usize = 1024;
const MB: usize = KB * 1024;
const GB: usize = MB * 1024;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

#[derive(Debug)]
pub struct ConnectionOptions;

impl diesel::r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for ConnectionOptions
{
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        (|| {
            conn.batch_execute(&format!("PRAGMA busy_timeout = {};", Duration::from_secs(30).as_millis()))?;
            conn.batch_execute("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
            conn.batch_execute("PRAGMA foreign_keys = ON;")?;
            conn.batch_execute("PRAGMA temp_store = MEMORY;")?;
            conn.batch_execute(&format!("PRAGMA cache_size = {};", format_cache_size(2 * GB)))?;
            conn.batch_execute(&format!("PRAGMA mmap_size = {};", (4 * GB).to_string()))?;
            // TODO: need to run optimize before close as well
            conn.batch_execute("PRAGMA optimize=0x10002;")?;
            Ok(())
        })()
        .map_err(diesel::r2d2::Error::QueryError)
    }
}

impl Database {
    #[tracing::instrument(name="Database::new", err(Debug))]
    pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
        let manager = ConnectionManager::<SqliteConnection>::new(path.to_string_lossy());
        let pool = Pool::builder()
            .connection_customizer(Box::new(ConnectionOptions))
            .connection_timeout(Duration::from_secs(30))
            .test_on_check_out(true)
            .max_size(69)
            .min_idle(Some(10))
            .build(manager)?;
        // let pool = SqlitePoolOptions::default()
        //     .max_connections(1000)
        //     .min_connections(2)
        //     .max_lifetime(Duration::from_secs(3600))
        //     .connect_with(
        //         sqlx::sqlite::SqliteConnectOptions::new()
        //             .statement_cache_capacity(500)
        //             .pragma("cache_size", format_cache_size(2 * GB))
        //             .pragma("temp_store", "memory")
        //             .pragma("mmap_size", (4 * GB).to_string())
        //             .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        //             .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        //             .optimize_on_close(true, 1000)
        //             .busy_timeout(Duration::from_secs(20))
        //             .filename(path)
        //             .log_statements(LevelFilter::Debug)
        //             .log_slow_statements(LevelFilter::Warn, Duration::from_secs(2))
        //             .create_if_missing(true),
        //     )
        //     .await?;

        // if let Err(err) = sqlx::migrate!("./migrations").run(&pool).await {
        //     error!("error running migrations: {err}");
        // }

        let database = Self { pool };

        database.health_check()?;
        database.run_migrations()?;
        database.vacuum()?;

        Ok(database)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    fn health_check(&self) -> Result<(), DatabaseError> {
        // ensures that the math extensions are activated
        let result = diesel::sql_query("SELECT sin(0)").execute(&mut self.pool.get()?)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    fn vacuum(&self) -> Result<(), DatabaseError> {
        let result = diesel::sql_query("VACUUM").execute(&mut self.pool.get()?)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    fn run_migrations(&self) -> Result<(), DatabaseError>  {
        let conn = &mut self.pool.get()?;
        conn.run_pending_migrations(MIGRATIONS).map_err(|err| {
            anyhow::anyhow!(err.to_string())
        })?;

        Ok(())
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
