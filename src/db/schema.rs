use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Create all database tables if they don't exist
pub fn init_database(conn: &Connection) -> Result<()> {
    // Create shows table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS shows (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            alternate TEXT NOT NULL,
            season INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL DEFAULT 'subsplease',
            quality TEXT NOT NULL DEFAULT '1080p',
            download_path TEXT,
            last_downloaded_episode INTEGER DEFAULT 0,
            last_downloaded_hash TEXT,
            is_tracked INTEGER NOT NULL DEFAULT 1,
            latest_episode TEXT,
            next_air_date TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
        [],
    )
    .context("Failed to create shows table")?;

    // Create rss_config table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS rss_config (
            id INTEGER PRIMARY KEY,
            poll_times_per_day INTEGER NOT NULL DEFAULT 4,
            last_poll_time TEXT,
            enabled INTEGER NOT NULL DEFAULT 1
        )",
        [],
    )
    .context("Failed to create rss_config table")?;

    // Create download_history table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS download_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            show_id INTEGER NOT NULL,
            episode INTEGER NOT NULL,
            info_hash TEXT NOT NULL UNIQUE,
            torrent_url TEXT,
            downloaded_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (show_id) REFERENCES shows(id) ON DELETE CASCADE
        )",
        [],
    )
    .context("Failed to create download_history table")?;

    // Insert default RSS config if it doesn't exist
    conn.execute(
        "INSERT OR IGNORE INTO rss_config (id, poll_times_per_day, enabled) VALUES (1, 4, 1)",
        [],
    )
    .context("Failed to insert default RSS config")?;

    Ok(())
}

/// Legacy TableEntry from tracked_shows.json
#[derive(Debug, Clone, Deserialize, Serialize)]
struct LegacyTableEntry {
    pub title: String,
    pub latest_episode: String,
    pub next_air_date: String,
    pub is_tracked: bool,
    pub id: u32,
}

/// Legacy TrackerDataEntry from tracked_data.json
#[derive(Debug, Clone, Deserialize, Serialize)]
struct LegacyTrackerDataEntry {
    pub title: String,
    pub id: u32,
    pub alternate: String,
    pub season: u8,
    pub source: String,
}

/// Migrate data from existing JSON files to the SQLite database
///
/// This reads `tracked_shows.json` and `tracked_data.json`, merges them
/// into the shows table, and renames the JSON files to `.bak`
pub fn migrate_from_json_if_needed(conn: &Connection) -> Result<()> {
    let shows_path = Path::new("tracked_shows.json");
    let data_path = Path::new("tracked_data.json");

    // Check if any JSON files exist
    let shows_exists = shows_path.exists();
    let data_exists = data_path.exists();

    if !shows_exists && !data_exists {
        tracing::info!("No JSON files to migrate");
        return Ok(());
    }

    tracing::info!("Starting migration from JSON files...");

    // Read tracked_shows.json
    let table_entries: HashMap<u32, LegacyTableEntry> = if shows_exists {
        let content = std::fs::read_to_string(shows_path)
            .context("Failed to read tracked_shows.json")?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Read tracked_data.json
    let data_entries: HashMap<u32, LegacyTrackerDataEntry> = if data_exists {
        let content = std::fs::read_to_string(data_path)
            .context("Failed to read tracked_data.json")?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Collect all unique IDs from both maps
    let mut all_ids: std::collections::HashSet<u32> = table_entries.keys().copied().collect();
    all_ids.extend(data_entries.keys().copied());

    let mut migrated_count = 0;

    for id in all_ids {
        let table_entry = table_entries.get(&id);
        let data_entry = data_entries.get(&id);

        // Build the merged show record
        let title = data_entry
            .map(|d| d.title.clone())
            .or_else(|| table_entry.map(|t| t.title.clone()))
            .unwrap_or_default();

        let alternate = data_entry
            .map(|d| d.alternate.clone())
            .unwrap_or_else(|| title.clone());

        let season = data_entry.map(|d| d.season).unwrap_or(1);

        let source = data_entry
            .map(|d| d.source.clone())
            .unwrap_or_else(|| "subsplease".to_string());

        let is_tracked = table_entry.map(|t| t.is_tracked).unwrap_or(true);

        let latest_episode = table_entry.map(|t| t.latest_episode.clone());
        let next_air_date = table_entry.map(|t| t.next_air_date.clone());

        // Insert into database (or update if exists)
        conn.execute(
            "INSERT INTO shows (id, title, alternate, season, source, quality, is_tracked, latest_episode, next_air_date)
             VALUES (?1, ?2, ?3, ?4, ?5, '1080p', ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                alternate = excluded.alternate,
                season = excluded.season,
                source = excluded.source,
                is_tracked = excluded.is_tracked,
                latest_episode = excluded.latest_episode,
                next_air_date = excluded.next_air_date,
                updated_at = datetime('now')",
            rusqlite::params![
                id,
                title,
                alternate,
                season,
                source,
                is_tracked as i32,
                latest_episode,
                next_air_date
            ],
        )
        .with_context(|| format!("Failed to migrate show with id {}", id))?;

        migrated_count += 1;
    }

    tracing::info!("Migrated {} shows from JSON files", migrated_count);

    // Rename JSON files to .bak
    if shows_exists {
        let backup_path = Path::new("tracked_shows.json.bak");
        std::fs::rename(shows_path, backup_path)
            .context("Failed to rename tracked_shows.json to .bak")?;
        tracing::info!("Renamed tracked_shows.json to tracked_shows.json.bak");
    }

    if data_exists {
        let backup_path = Path::new("tracked_data.json.bak");
        std::fs::rename(data_path, backup_path)
            .context("Failed to rename tracked_data.json to .bak")?;
        tracing::info!("Renamed tracked_data.json to tracked_data.json.bak");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_init_database() {
        let conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"shows".to_string()));
        assert!(tables.contains(&"rss_config".to_string()));
        assert!(tables.contains(&"download_history".to_string()));
    }

    #[test]
    fn test_default_rss_config() {
        let conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();

        let (poll_times, enabled): (i32, i32) = conn
            .query_row(
                "SELECT poll_times_per_day, enabled FROM rss_config WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(poll_times, 4);
        assert_eq!(enabled, 1);
    }
}
