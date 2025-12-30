use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use super::models::Show;

/// Get all shows from the database
pub fn get_all_shows(conn: &Connection) -> Result<Vec<Show>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, alternate, season, source, quality, download_path,
                    last_downloaded_episode, last_downloaded_hash, is_tracked,
                    latest_episode, next_air_date, created_at, updated_at
             FROM shows
             ORDER BY title",
        )
        .context("Failed to prepare get_all_shows query")?;

    let shows = stmt
        .query_map([], |row| {
            Ok(Show {
                id: row.get(0)?,
                title: row.get(1)?,
                alternate: row.get(2)?,
                season: row.get::<_, i32>(3)? as u8,
                source: row.get(4)?,
                quality: row.get(5)?,
                download_path: row.get(6)?,
                last_downloaded_episode: row.get::<_, i32>(7)? as u16,
                last_downloaded_hash: row.get(8)?,
                is_tracked: row.get::<_, i32>(9)? != 0,
                latest_episode: row.get(10)?,
                next_air_date: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })
        .context("Failed to execute get_all_shows query")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to collect shows")?;

    Ok(shows)
}

/// Get all tracked shows (where is_tracked = 1)
pub fn get_tracked_shows(conn: &Connection) -> Result<Vec<Show>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, alternate, season, source, quality, download_path,
                    last_downloaded_episode, last_downloaded_hash, is_tracked,
                    latest_episode, next_air_date, created_at, updated_at
             FROM shows
             WHERE is_tracked = 1
             ORDER BY title",
        )
        .context("Failed to prepare get_tracked_shows query")?;

    let shows = stmt
        .query_map([], |row| {
            Ok(Show {
                id: row.get(0)?,
                title: row.get(1)?,
                alternate: row.get(2)?,
                season: row.get::<_, i32>(3)? as u8,
                source: row.get(4)?,
                quality: row.get(5)?,
                download_path: row.get(6)?,
                last_downloaded_episode: row.get::<_, i32>(7)? as u16,
                last_downloaded_hash: row.get(8)?,
                is_tracked: row.get::<_, i32>(9)? != 0,
                latest_episode: row.get(10)?,
                next_air_date: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })
        .context("Failed to execute get_tracked_shows query")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to collect tracked shows")?;

    Ok(shows)
}

/// Get a single show by ID
pub fn get_show(conn: &Connection, id: u32) -> Result<Option<Show>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, alternate, season, source, quality, download_path,
                    last_downloaded_episode, last_downloaded_hash, is_tracked,
                    latest_episode, next_air_date, created_at, updated_at
             FROM shows
             WHERE id = ?1",
        )
        .context("Failed to prepare get_show query")?;

    let show = stmt
        .query_row([id], |row| {
            Ok(Show {
                id: row.get(0)?,
                title: row.get(1)?,
                alternate: row.get(2)?,
                season: row.get::<_, i32>(3)? as u8,
                source: row.get(4)?,
                quality: row.get(5)?,
                download_path: row.get(6)?,
                last_downloaded_episode: row.get::<_, i32>(7)? as u16,
                last_downloaded_hash: row.get(8)?,
                is_tracked: row.get::<_, i32>(9)? != 0,
                latest_episode: row.get(10)?,
                next_air_date: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })
        .optional()
        .context("Failed to execute get_show query")?;

    Ok(show)
}

/// Insert a new show into the database
pub fn insert_show(conn: &Connection, show: &Show) -> Result<()> {
    conn.execute(
        "INSERT INTO shows (id, title, alternate, season, source, quality, download_path,
                           last_downloaded_episode, last_downloaded_hash, is_tracked,
                           latest_episode, next_air_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            show.id,
            show.title,
            show.alternate,
            show.season as i32,
            show.source,
            show.quality,
            show.download_path,
            show.last_downloaded_episode as i32,
            show.last_downloaded_hash,
            show.is_tracked as i32,
            show.latest_episode,
            show.next_air_date,
        ],
    )
    .context("Failed to insert show")?;

    Ok(())
}

/// Update an existing show in the database
pub fn update_show(conn: &Connection, show: &Show) -> Result<()> {
    conn.execute(
        "UPDATE shows SET
            title = ?2,
            alternate = ?3,
            season = ?4,
            source = ?5,
            quality = ?6,
            download_path = ?7,
            last_downloaded_episode = ?8,
            last_downloaded_hash = ?9,
            is_tracked = ?10,
            latest_episode = ?11,
            next_air_date = ?12,
            updated_at = datetime('now')
         WHERE id = ?1",
        params![
            show.id,
            show.title,
            show.alternate,
            show.season as i32,
            show.source,
            show.quality,
            show.download_path,
            show.last_downloaded_episode as i32,
            show.last_downloaded_hash,
            show.is_tracked as i32,
            show.latest_episode,
            show.next_air_date,
        ],
    )
    .context("Failed to update show")?;

    Ok(())
}

/// Delete a show by ID
pub fn delete_show(conn: &Connection, id: u32) -> Result<()> {
    conn.execute("DELETE FROM shows WHERE id = ?1", [id])
        .context("Failed to delete show")?;

    Ok(())
}

/// Update the last downloaded episode and hash for a show
pub fn update_last_downloaded(conn: &Connection, id: u32, episode: u16, hash: &str) -> Result<()> {
    conn.execute(
        "UPDATE shows SET
            last_downloaded_episode = ?2,
            last_downloaded_hash = ?3,
            updated_at = datetime('now')
         WHERE id = ?1",
        params![id, episode as i32, hash],
    )
    .context("Failed to update last downloaded")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::init_database;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();
        conn
    }

    #[test]
    fn test_insert_and_get_show() {
        let conn = setup_test_db();

        let show = Show {
            id: 12345,
            title: "Test Anime".to_string(),
            alternate: "Test Anime Alt".to_string(),
            season: 1,
            source: "subsplease".to_string(),
            quality: "1080p".to_string(),
            download_path: None,
            last_downloaded_episode: 0,
            last_downloaded_hash: None,
            is_tracked: true,
            latest_episode: Some("Episode 5".to_string()),
            next_air_date: Some("2024-01-15".to_string()),
            created_at: None,
            updated_at: None,
        };

        insert_show(&conn, &show).unwrap();

        let retrieved = get_show(&conn, 12345).unwrap().unwrap();
        assert_eq!(retrieved.title, "Test Anime");
        assert_eq!(retrieved.alternate, "Test Anime Alt");
        assert!(retrieved.is_tracked);
    }

    #[test]
    fn test_get_tracked_shows() {
        let conn = setup_test_db();

        let show1 = Show {
            id: 1,
            title: "Tracked Show".to_string(),
            is_tracked: true,
            ..Default::default()
        };

        let show2 = Show {
            id: 2,
            title: "Untracked Show".to_string(),
            is_tracked: false,
            ..Default::default()
        };

        insert_show(&conn, &show1).unwrap();
        insert_show(&conn, &show2).unwrap();

        let tracked = get_tracked_shows(&conn).unwrap();
        assert_eq!(tracked.len(), 1);
        assert_eq!(tracked[0].title, "Tracked Show");
    }

    #[test]
    fn test_update_show() {
        let conn = setup_test_db();

        let mut show = Show {
            id: 1,
            title: "Original Title".to_string(),
            ..Default::default()
        };

        insert_show(&conn, &show).unwrap();

        show.title = "Updated Title".to_string();
        show.season = 2;
        update_show(&conn, &show).unwrap();

        let retrieved = get_show(&conn, 1).unwrap().unwrap();
        assert_eq!(retrieved.title, "Updated Title");
        assert_eq!(retrieved.season, 2);
    }

    #[test]
    fn test_delete_show() {
        let conn = setup_test_db();

        let show = Show {
            id: 1,
            title: "To Delete".to_string(),
            ..Default::default()
        };

        insert_show(&conn, &show).unwrap();
        assert!(get_show(&conn, 1).unwrap().is_some());

        delete_show(&conn, 1).unwrap();
        assert!(get_show(&conn, 1).unwrap().is_none());
    }

    #[test]
    fn test_update_last_downloaded() {
        let conn = setup_test_db();

        let show = Show {
            id: 1,
            title: "Test Show".to_string(),
            ..Default::default()
        };

        insert_show(&conn, &show).unwrap();

        update_last_downloaded(&conn, 1, 5, "abc123hash").unwrap();

        let retrieved = get_show(&conn, 1).unwrap().unwrap();
        assert_eq!(retrieved.last_downloaded_episode, 5);
        assert_eq!(retrieved.last_downloaded_hash, Some("abc123hash".to_string()));
    }
}
