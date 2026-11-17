//! SQLite adapter for SlimLocalize.
//!
//! The adapter is intentionally concrete: SQLite is the only persistence
//! backend implemented in F1. The domain talks to it through the `Store` port,
//! never through SQL strings.

mod collab;
mod migrations;
mod store;

pub use store::{SqliteStore, StorageStats};

/// OS-appropriate data directory for the desktop app and CLI.
///
/// Linux: `~/.local/share/slimlocalize`, macOS: `~/Library/Application Support/dev.slimlocalize`,
/// Windows: `%APPDATA%\SlimLocalize\SlimLocalize`. `SLIMLOCALIZE_DB` always
/// takes precedence when the adapters read it.
#[must_use]
pub fn default_database_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("dev", "SlimLocalize", "SlimLocalize")
        .map(|dirs| dirs.data_dir().join("slimlocalize.sqlite3"))
        .unwrap_or_else(|| std::path::PathBuf::from("slimlocalize.sqlite3"))
}
