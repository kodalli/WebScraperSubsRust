pub mod config;
pub mod history;
pub mod schema;
pub mod shows;

use std::sync::OnceLock;

use anyhow::{Context, Result};
use rusqlite::Connection;
use tokio::sync::Mutex;

/// Global database connection wrapped in a Mutex for thread-safe access
static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

/// Get the database file path from environment or use default
fn get_db_path() -> String {
    std::env::var("DATABASE_PATH").unwrap_or_else(|_| "tracker.db".to_string())
}

/// Initialize the global database connection
pub fn init_connection() -> Result<()> {
    let conn = Connection::open(&get_db_path()).context("Failed to open database connection")?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])
        .context("Failed to enable foreign keys")?;

    DB.set(Mutex::new(conn))
        .map_err(|_| anyhow::anyhow!("Database already initialized"))?;

    Ok(())
}

/// Get a reference to the global database connection mutex
pub fn get_connection() -> Result<&'static Mutex<Connection>> {
    DB.get()
        .ok_or_else(|| anyhow::anyhow!("Database not initialized. Call init_connection() first."))
}

/// Execute a blocking database operation using spawn_blocking
pub async fn with_db<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&Connection) -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let db = get_connection()?;
    let conn = db.lock().await;

    // Since rusqlite is sync, we need to use spawn_blocking
    // But we already have the lock, so we execute directly
    // The mutex ensures thread safety
    f(&conn)
}

/// Execute a blocking database operation that requires mutable access
pub async fn with_db_mut<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let db = get_connection()?;
    let mut conn = db.lock().await;

    f(&mut conn)
}

// Re-export commonly used types and functions
pub use config::{get_rss_config, set_rss_enabled, update_last_poll_time, update_poll_interval};
pub use history::{get_show_history, is_already_downloaded, record_download};
pub use schema::{init_database, migrate_from_json_if_needed};
pub use shows::{
    delete_show, get_all_shows, get_show, get_tracked_shows, insert_show, update_last_downloaded,
    update_show,
};

/// Data models for the database layer
pub mod models {
    use serde::{Deserialize, Serialize};

    /// Represents a show in the database
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Show {
        pub id: u32,
        pub title: String,
        pub alternate: String,
        pub season: u8,
        pub source: String,
        pub quality: String,
        pub download_path: Option<String>,
        pub last_downloaded_episode: u16,
        pub last_downloaded_hash: Option<String>,
        pub is_tracked: bool,
        pub latest_episode: Option<String>,
        pub next_air_date: Option<String>,
        pub created_at: Option<String>,
        pub updated_at: Option<String>,
    }

    impl Default for Show {
        fn default() -> Self {
            Self {
                id: 0,
                title: String::new(),
                alternate: String::new(),
                season: 1,
                source: "subsplease".to_string(),
                quality: "1080p".to_string(),
                download_path: None,
                last_downloaded_episode: 0,
                last_downloaded_hash: None,
                is_tracked: true,
                latest_episode: None,
                next_air_date: None,
                created_at: None,
                updated_at: None,
            }
        }
    }

    /// RSS configuration
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RssConfig {
        pub id: u32,
        pub poll_times_per_day: u8,
        pub last_poll_time: Option<String>,
        pub enabled: bool,
    }

    impl Default for RssConfig {
        fn default() -> Self {
            Self {
                id: 1,
                poll_times_per_day: 4,
                last_poll_time: None,
                enabled: true,
            }
        }
    }

    /// A record of a downloaded episode
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DownloadRecord {
        pub id: u32,
        pub show_id: u32,
        pub episode: u16,
        pub info_hash: String,
        pub torrent_url: Option<String>,
        pub downloaded_at: Option<String>,
    }
}
