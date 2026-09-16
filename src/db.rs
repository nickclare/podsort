#![allow(unused)] //temporary
use std::path::Path;

use color_eyre::eyre::{self, Context};
use rusqlite::Connection;

/// Wrapper for the database of podcasts
pub struct PodcastDb {
    conn: Connection,
}

pub const SCHEMA_VERSION: &str = "v0.0.1-01";

const SCHEMA_DDL: &str = indoc::indoc! {"

    CREATE TABLE IF NOT EXISTS podsort_info (
        key TEXT PRIMARY_KEY,
        value TEXT
    );

    CREATE TABLE podcasts (
        id TEXT PRIMARY_KEY,
        feed_url TEXT NOT NULL,
        last_updated TEXT,
        feed_data TEXT
    );

"};

impl PodcastDb {
    pub fn from_path(path: impl AsRef<Path>) -> eyre::Result<Self> {
        let conn = Connection::open(path).wrap_err("unable to open sqlite database")?;
        Self::from_connection(conn)
    }

    pub fn from_memory() -> eyre::Result<Self> {
        let conn = Connection::open_in_memory().wrap_err("unable to open sqlite in-memory")?;
        Self::from_connection(conn)
    }

    pub fn from_connection(conn: Connection) -> eyre::Result<Self> {
        let mut db = Self { conn };
        db.init()?;
        Ok(db)
    }

    fn init(&mut self) -> eyre::Result<()> {
        // check if initialized already
        if self.is_initialized()? {
            tracing::debug!("db already initialized");
            Ok(())
        } else {
            self.conn
                .execute_batch(SCHEMA_DDL)
                .wrap_err("error initializing database")?;
            Ok(())
        }
    }

    fn is_initialized(&mut self) -> eyre::Result<bool> {
        //TODO: should this return a custom enum instead?
        if self.conn.table_exists(None, "podsort_info")? {
            let schema_version: String = self.conn.query_row_and_then(
                "SELECT value FROM podsort_info WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )?;
            if schema_version != SCHEMA_VERSION {
                eyre::bail!(
                    "expected schema version {SCHEMA_VERSION}, got {schema_version} instead"
                );
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn count_podcasts(&self) -> eyre::Result<u32> {
        Ok(self
            .conn
            .query_one("SELECT count(*) from podcasts", [], |row| row.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_db_init() -> eyre::Result<()> {
        let _db = PodcastDb::from_memory()?;
        assert!(_db.count_podcasts()? == 0);
        Ok(())
    }
}
