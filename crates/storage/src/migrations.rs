use std::time::Duration;

use rusqlite::Connection;
use slimlocalize_core::{DomainError, Result};

/// Ordered, append-only schema migrations embedded in the binary.
///
/// Never edit a migration that has shipped; add a new version instead.
pub const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../migrations/0001_init.sql")),
    (2, include_str!("../migrations/0002_collaboration.sql")),
];

pub fn configure(connection: &Connection) -> Result<()> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(map_db("configure busy timeout"))?;
    // WAL is the right default for a local single-writer application. The
    // in-memory test database reports "memory" instead and is still fine.
    let _: String = connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(map_db("enable WAL"))?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(map_db("enable foreign keys"))?;
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(map_db("set synchronous mode"))?;
    connection
        .pragma_update(None, "journal_size_limit", 64 * 1024 * 1024_i64)
        .map_err(map_db("set journal size limit"))?;
    connection
        .pragma_update(None, "temp_store", "MEMORY")
        .map_err(map_db("set temp store"))?;
    Ok(())
}

pub fn apply(connection: &mut Connection) -> Result<i64> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY NOT NULL,
                applied_at TEXT NOT NULL
            );",
        )
        .map_err(map_db("create schema_migrations"))?;

    let mut current: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(map_db("read schema version"))?;

    for (version, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }
        let tx = connection
            .transaction()
            .map_err(map_db("begin migration transaction"))?;
        tx.execute_batch(sql)
            .map_err(map_db(&format!("apply migration {version}")))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![version, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(map_db("record migration"))?;
        tx.commit()
            .map_err(map_db("commit migration transaction"))?;
        current = *version;
        tracing::info!(version, "applied database migration");
    }

    Ok(current)
}

pub fn map_db(context: &str) -> impl FnOnce(rusqlite::Error) -> DomainError + '_ {
    move |source| DomainError::Storage {
        message: format!("{context}: {source}"),
    }
}
