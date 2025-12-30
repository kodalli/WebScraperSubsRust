use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use super::models::DownloadRecord;

/// Check if a torrent has already been downloaded by its info hash
pub fn is_already_downloaded(conn: &Connection, info_hash: &str) -> Result<bool> {
    let exists: Option<i32> = conn
        .query_row(
            "SELECT 1 FROM download_history WHERE info_hash = ?1 LIMIT 1",
            params![info_hash],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to check if already downloaded")?;

    Ok(exists.is_some())
}

/// Record a new download in the history
pub fn record_download(
    conn: &Connection,
    show_id: u32,
    episode: u16,
    hash: &str,
    url: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO download_history (show_id, episode, info_hash, torrent_url)
         VALUES (?1, ?2, ?3, ?4)",
        params![show_id, episode as i32, hash, url],
    )
    .context("Failed to record download")?;

    Ok(())
}

/// Get the download history for a specific show
pub fn get_show_history(conn: &Connection, show_id: u32) -> Result<Vec<DownloadRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, show_id, episode, info_hash, torrent_url, downloaded_at
             FROM download_history
             WHERE show_id = ?1
             ORDER BY downloaded_at DESC",
        )
        .context("Failed to prepare get_show_history query")?;

    let records = stmt
        .query_map([show_id], |row| {
            Ok(DownloadRecord {
                id: row.get(0)?,
                show_id: row.get(1)?,
                episode: row.get::<_, i32>(2)? as u16,
                info_hash: row.get(3)?,
                torrent_url: row.get(4)?,
                downloaded_at: row.get(5)?,
            })
        })
        .context("Failed to execute get_show_history query")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to collect download records")?;

    Ok(records)
}

/// Get all download history (useful for debugging/admin)
pub fn get_all_history(conn: &Connection) -> Result<Vec<DownloadRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, show_id, episode, info_hash, torrent_url, downloaded_at
             FROM download_history
             ORDER BY downloaded_at DESC",
        )
        .context("Failed to prepare get_all_history query")?;

    let records = stmt
        .query_map([], |row| {
            Ok(DownloadRecord {
                id: row.get(0)?,
                show_id: row.get(1)?,
                episode: row.get::<_, i32>(2)? as u16,
                info_hash: row.get(3)?,
                torrent_url: row.get(4)?,
                downloaded_at: row.get(5)?,
            })
        })
        .context("Failed to execute get_all_history query")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to collect download records")?;

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::init_database;
    use crate::db::shows::insert_show;
    use crate::db::models::Show;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();

        // Insert a test show for foreign key constraints
        let show = Show {
            id: 1,
            title: "Test Show".to_string(),
            ..Default::default()
        };
        insert_show(&conn, &show).unwrap();

        conn
    }

    #[test]
    fn test_is_already_downloaded_false() {
        let conn = setup_test_db();

        let result = is_already_downloaded(&conn, "nonexistent_hash").unwrap();
        assert!(!result);
    }

    #[test]
    fn test_record_and_check_download() {
        let conn = setup_test_db();

        record_download(&conn, 1, 5, "test_hash_123", "http://example.com/torrent").unwrap();

        let result = is_already_downloaded(&conn, "test_hash_123").unwrap();
        assert!(result);
    }

    #[test]
    fn test_get_show_history() {
        let conn = setup_test_db();

        record_download(&conn, 1, 1, "hash1", "http://example.com/1").unwrap();
        record_download(&conn, 1, 2, "hash2", "http://example.com/2").unwrap();
        record_download(&conn, 1, 3, "hash3", "http://example.com/3").unwrap();

        let history = get_show_history(&conn, 1).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_unique_hash_constraint() {
        let conn = setup_test_db();

        record_download(&conn, 1, 1, "unique_hash", "http://example.com/1").unwrap();

        // Attempting to insert the same hash should fail
        let result = record_download(&conn, 1, 2, "unique_hash", "http://example.com/2");
        assert!(result.is_err());
    }

    #[test]
    fn test_cascade_delete() {
        let conn = setup_test_db();

        record_download(&conn, 1, 1, "cascade_test_hash", "http://example.com/1").unwrap();

        // Delete the show
        conn.execute("DELETE FROM shows WHERE id = 1", []).unwrap();

        // History should be deleted too
        let history = get_all_history(&conn).unwrap();
        assert!(history.is_empty());
    }
}
