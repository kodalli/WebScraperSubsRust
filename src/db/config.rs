use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use super::models::RssConfig;

/// Get the RSS configuration (there's only one row with id=1)
pub fn get_rss_config(conn: &Connection) -> Result<RssConfig> {
    let config = conn
        .query_row(
            "SELECT id, poll_times_per_day, last_poll_time, enabled
             FROM rss_config
             WHERE id = 1",
            [],
            |row| {
                Ok(RssConfig {
                    id: row.get(0)?,
                    poll_times_per_day: row.get::<_, i32>(1)? as u8,
                    last_poll_time: row.get(2)?,
                    enabled: row.get::<_, i32>(3)? != 0,
                })
            },
        )
        .context("Failed to get RSS config")?;

    Ok(config)
}

/// Update the poll interval (times per day)
pub fn update_poll_interval(conn: &Connection, times_per_day: u8) -> Result<()> {
    conn.execute(
        "UPDATE rss_config SET poll_times_per_day = ?1 WHERE id = 1",
        params![times_per_day as i32],
    )
    .context("Failed to update poll interval")?;

    Ok(())
}

/// Update the last poll time to the current time
pub fn update_last_poll_time(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE rss_config SET last_poll_time = datetime('now') WHERE id = 1",
        [],
    )
    .context("Failed to update last poll time")?;

    Ok(())
}

/// Enable or disable RSS polling
pub fn set_rss_enabled(conn: &Connection, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE rss_config SET enabled = ?1 WHERE id = 1",
        params![enabled as i32],
    )
    .context("Failed to set RSS enabled state")?;

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
    fn test_get_default_rss_config() {
        let conn = setup_test_db();

        let config = get_rss_config(&conn).unwrap();
        assert_eq!(config.id, 1);
        assert_eq!(config.poll_times_per_day, 4);
        assert!(config.enabled);
        assert!(config.last_poll_time.is_none());
    }

    #[test]
    fn test_update_poll_interval() {
        let conn = setup_test_db();

        update_poll_interval(&conn, 12).unwrap();

        let config = get_rss_config(&conn).unwrap();
        assert_eq!(config.poll_times_per_day, 12);
    }

    #[test]
    fn test_update_last_poll_time() {
        let conn = setup_test_db();

        update_last_poll_time(&conn).unwrap();

        let config = get_rss_config(&conn).unwrap();
        assert!(config.last_poll_time.is_some());
    }

    #[test]
    fn test_set_rss_enabled() {
        let conn = setup_test_db();

        // Initially enabled
        let config = get_rss_config(&conn).unwrap();
        assert!(config.enabled);

        // Disable
        set_rss_enabled(&conn, false).unwrap();
        let config = get_rss_config(&conn).unwrap();
        assert!(!config.enabled);

        // Re-enable
        set_rss_enabled(&conn, true).unwrap();
        let config = get_rss_config(&conn).unwrap();
        assert!(config.enabled);
    }
}
