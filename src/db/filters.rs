//! Filter rules database operations
//!
//! Provides CRUD operations for filter rules and show-specific filter overrides.

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Filter type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterType {
    Resolution,
    Group,
    TitleExclude,
    TitleInclude,
}

impl FilterType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FilterType::Resolution => "resolution",
            FilterType::Group => "group",
            FilterType::TitleExclude => "title_exclude",
            FilterType::TitleInclude => "title_include",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "resolution" => Some(FilterType::Resolution),
            "group" => Some(FilterType::Group),
            "title_exclude" => Some(FilterType::TitleExclude),
            "title_include" => Some(FilterType::TitleInclude),
            _ => None,
        }
    }
}

/// Filter action enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterAction {
    Prefer,
    Require,
    Exclude,
}

impl FilterAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            FilterAction::Prefer => "prefer",
            FilterAction::Require => "require",
            FilterAction::Exclude => "exclude",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "prefer" => Some(FilterAction::Prefer),
            "require" => Some(FilterAction::Require),
            "exclude" => Some(FilterAction::Exclude),
            _ => None,
        }
    }
}

/// Represents a filter rule in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterRule {
    pub id: u32,
    pub name: String,
    pub filter_type: FilterType,
    pub pattern: String,
    pub action: FilterAction,
    pub priority: i32,
    pub is_global: bool,
    pub enabled: bool,
    pub created_at: Option<String>,
}

/// Represents a show-specific filter override
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowFilterOverride {
    pub id: u32,
    pub show_id: u32,
    pub filter_rule_id: Option<u32>,
    pub filter_type: Option<FilterType>,
    pub pattern: Option<String>,
    pub action: FilterAction,
    pub enabled: bool,
}

/// Input for creating a new filter rule
#[derive(Debug, Clone, Deserialize)]
pub struct CreateFilterRule {
    pub name: String,
    pub filter_type: String,
    pub pattern: String,
    pub action: String,
    pub priority: i32,
}

/// Input for updating a filter rule
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateFilterRule {
    pub name: Option<String>,
    pub filter_type: Option<String>,
    pub pattern: Option<String>,
    pub action: Option<String>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
}

/// Get all global filter rules
pub fn get_global_filters(conn: &Connection) -> Result<Vec<FilterRule>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, filter_type, pattern, action, priority, is_global, enabled, created_at
             FROM filter_rules
             WHERE is_global = 1
             ORDER BY priority DESC, id ASC",
        )
        .context("Failed to prepare get_global_filters statement")?;

    let rows = stmt
        .query_map([], |row| {
            Ok(FilterRule {
                id: row.get(0)?,
                name: row.get(1)?,
                filter_type: FilterType::from_str(&row.get::<_, String>(2)?)
                    .unwrap_or(FilterType::Resolution),
                pattern: row.get(3)?,
                action: FilterAction::from_str(&row.get::<_, String>(4)?)
                    .unwrap_or(FilterAction::Prefer),
                priority: row.get(5)?,
                is_global: row.get::<_, i32>(6)? != 0,
                enabled: row.get::<_, i32>(7)? != 0,
                created_at: row.get(8)?,
            })
        })
        .context("Failed to query global filters")?;

    let mut filters = Vec::new();
    for row in rows {
        filters.push(row.context("Failed to read filter row")?);
    }

    Ok(filters)
}

/// Get all filter rules (global and non-global)
pub fn get_all_filters(conn: &Connection) -> Result<Vec<FilterRule>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, filter_type, pattern, action, priority, is_global, enabled, created_at
             FROM filter_rules
             ORDER BY priority DESC, id ASC",
        )
        .context("Failed to prepare get_all_filters statement")?;

    let rows = stmt
        .query_map([], |row| {
            Ok(FilterRule {
                id: row.get(0)?,
                name: row.get(1)?,
                filter_type: FilterType::from_str(&row.get::<_, String>(2)?)
                    .unwrap_or(FilterType::Resolution),
                pattern: row.get(3)?,
                action: FilterAction::from_str(&row.get::<_, String>(4)?)
                    .unwrap_or(FilterAction::Prefer),
                priority: row.get(5)?,
                is_global: row.get::<_, i32>(6)? != 0,
                enabled: row.get::<_, i32>(7)? != 0,
                created_at: row.get(8)?,
            })
        })
        .context("Failed to query all filters")?;

    let mut filters = Vec::new();
    for row in rows {
        filters.push(row.context("Failed to read filter row")?);
    }

    Ok(filters)
}

/// Get a single filter by ID
pub fn get_filter(conn: &Connection, id: u32) -> Result<Option<FilterRule>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, filter_type, pattern, action, priority, is_global, enabled, created_at
             FROM filter_rules
             WHERE id = ?1",
        )
        .context("Failed to prepare get_filter statement")?;

    let result = stmt.query_row([id], |row| {
        Ok(FilterRule {
            id: row.get(0)?,
            name: row.get(1)?,
            filter_type: FilterType::from_str(&row.get::<_, String>(2)?)
                .unwrap_or(FilterType::Resolution),
            pattern: row.get(3)?,
            action: FilterAction::from_str(&row.get::<_, String>(4)?)
                .unwrap_or(FilterAction::Prefer),
            priority: row.get(5)?,
            is_global: row.get::<_, i32>(6)? != 0,
            enabled: row.get::<_, i32>(7)? != 0,
            created_at: row.get(8)?,
        })
    });

    match result {
        Ok(filter) => Ok(Some(filter)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e).context("Failed to get filter"),
    }
}

/// Create a new filter rule
pub fn create_filter(conn: &Connection, filter: &CreateFilterRule) -> Result<u32> {
    conn.execute(
        "INSERT INTO filter_rules (name, filter_type, pattern, action, priority, is_global, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, 1)",
        rusqlite::params![
            filter.name,
            filter.filter_type,
            filter.pattern,
            filter.action,
            filter.priority,
        ],
    )
    .context("Failed to insert filter rule")?;

    let id = conn.last_insert_rowid() as u32;
    Ok(id)
}

/// Update an existing filter rule
pub fn update_filter(conn: &Connection, id: u32, update: &UpdateFilterRule) -> Result<bool> {
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref name) = update.name {
        updates.push("name = ?");
        params.push(Box::new(name.clone()));
    }
    if let Some(ref filter_type) = update.filter_type {
        updates.push("filter_type = ?");
        params.push(Box::new(filter_type.clone()));
    }
    if let Some(ref pattern) = update.pattern {
        updates.push("pattern = ?");
        params.push(Box::new(pattern.clone()));
    }
    if let Some(ref action) = update.action {
        updates.push("action = ?");
        params.push(Box::new(action.clone()));
    }
    if let Some(priority) = update.priority {
        updates.push("priority = ?");
        params.push(Box::new(priority));
    }
    if let Some(enabled) = update.enabled {
        updates.push("enabled = ?");
        params.push(Box::new(enabled as i32));
    }

    if updates.is_empty() {
        return Ok(false);
    }

    params.push(Box::new(id));

    let sql = format!(
        "UPDATE filter_rules SET {} WHERE id = ?",
        updates.join(", ")
    );

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows_affected = conn
        .execute(&sql, params_refs.as_slice())
        .context("Failed to update filter rule")?;

    Ok(rows_affected > 0)
}

/// Delete a filter rule
pub fn delete_filter(conn: &Connection, id: u32) -> Result<bool> {
    let rows_affected = conn
        .execute("DELETE FROM filter_rules WHERE id = ?1", [id])
        .context("Failed to delete filter rule")?;

    Ok(rows_affected > 0)
}

/// Get filter overrides for a specific show
pub fn get_show_filters(conn: &Connection, show_id: u32) -> Result<Vec<ShowFilterOverride>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, show_id, filter_rule_id, filter_type, pattern, action, enabled
             FROM show_filter_overrides
             WHERE show_id = ?1",
        )
        .context("Failed to prepare get_show_filters statement")?;

    let rows = stmt
        .query_map([show_id], |row| {
            let filter_type_str: Option<String> = row.get(3)?;
            Ok(ShowFilterOverride {
                id: row.get(0)?,
                show_id: row.get(1)?,
                filter_rule_id: row.get(2)?,
                filter_type: filter_type_str.and_then(|s| FilterType::from_str(&s)),
                pattern: row.get(4)?,
                action: FilterAction::from_str(&row.get::<_, String>(5)?)
                    .unwrap_or(FilterAction::Prefer),
                enabled: row.get::<_, i32>(6)? != 0,
            })
        })
        .context("Failed to query show filters")?;

    let mut filters = Vec::new();
    for row in rows {
        filters.push(row.context("Failed to read show filter row")?);
    }

    Ok(filters)
}

/// Create a show-specific filter override
pub fn create_show_filter(
    conn: &Connection,
    show_id: u32,
    filter_rule_id: Option<u32>,
    filter_type: Option<&str>,
    pattern: Option<&str>,
    action: &str,
) -> Result<u32> {
    conn.execute(
        "INSERT INTO show_filter_overrides (show_id, filter_rule_id, filter_type, pattern, action, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        rusqlite::params![show_id, filter_rule_id, filter_type, pattern, action],
    )
    .context("Failed to insert show filter override")?;

    let id = conn.last_insert_rowid() as u32;
    Ok(id)
}

/// Delete a show-specific filter override
pub fn delete_show_filter(conn: &Connection, id: u32) -> Result<bool> {
    let rows_affected = conn
        .execute("DELETE FROM show_filter_overrides WHERE id = ?1", [id])
        .context("Failed to delete show filter override")?;

    Ok(rows_affected > 0)
}

/// Toggle a filter's enabled status
pub fn toggle_filter(conn: &Connection, id: u32) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "UPDATE filter_rules SET enabled = NOT enabled WHERE id = ?1",
            [id],
        )
        .context("Failed to toggle filter")?;

    Ok(rows_affected > 0)
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
    fn test_get_global_filters() {
        let conn = setup_test_db();
        let filters = get_global_filters(&conn).unwrap();
        // Should have default filters seeded
        assert!(!filters.is_empty());
    }

    #[test]
    fn test_create_and_get_filter() {
        let conn = setup_test_db();

        let new_filter = CreateFilterRule {
            name: "Test Filter".to_string(),
            filter_type: "group".to_string(),
            pattern: "TestGroup".to_string(),
            action: "exclude".to_string(),
            priority: 50,
        };

        let id = create_filter(&conn, &new_filter).unwrap();
        assert!(id > 0);

        let filter = get_filter(&conn, id).unwrap().unwrap();
        assert_eq!(filter.name, "Test Filter");
        assert_eq!(filter.filter_type, FilterType::Group);
        assert_eq!(filter.pattern, "TestGroup");
        assert_eq!(filter.action, FilterAction::Exclude);
        assert_eq!(filter.priority, 50);
    }

    #[test]
    fn test_update_filter() {
        let conn = setup_test_db();

        let new_filter = CreateFilterRule {
            name: "Original".to_string(),
            filter_type: "resolution".to_string(),
            pattern: "720p".to_string(),
            action: "prefer".to_string(),
            priority: 5,
        };

        let id = create_filter(&conn, &new_filter).unwrap();

        let update = UpdateFilterRule {
            name: Some("Updated".to_string()),
            priority: Some(15),
            filter_type: None,
            pattern: None,
            action: None,
            enabled: None,
        };

        let updated = update_filter(&conn, id, &update).unwrap();
        assert!(updated);

        let filter = get_filter(&conn, id).unwrap().unwrap();
        assert_eq!(filter.name, "Updated");
        assert_eq!(filter.priority, 15);
    }

    #[test]
    fn test_delete_filter() {
        let conn = setup_test_db();

        let new_filter = CreateFilterRule {
            name: "To Delete".to_string(),
            filter_type: "group".to_string(),
            pattern: "DeleteMe".to_string(),
            action: "exclude".to_string(),
            priority: 1,
        };

        let id = create_filter(&conn, &new_filter).unwrap();
        assert!(get_filter(&conn, id).unwrap().is_some());

        let deleted = delete_filter(&conn, id).unwrap();
        assert!(deleted);

        assert!(get_filter(&conn, id).unwrap().is_none());
    }

    #[test]
    fn test_toggle_filter() {
        let conn = setup_test_db();

        let new_filter = CreateFilterRule {
            name: "Toggle Test".to_string(),
            filter_type: "resolution".to_string(),
            pattern: "480p".to_string(),
            action: "exclude".to_string(),
            priority: 1,
        };

        let id = create_filter(&conn, &new_filter).unwrap();

        // Initially enabled
        let filter = get_filter(&conn, id).unwrap().unwrap();
        assert!(filter.enabled);

        // Toggle to disabled
        toggle_filter(&conn, id).unwrap();
        let filter = get_filter(&conn, id).unwrap().unwrap();
        assert!(!filter.enabled);

        // Toggle back to enabled
        toggle_filter(&conn, id).unwrap();
        let filter = get_filter(&conn, id).unwrap().unwrap();
        assert!(filter.enabled);
    }
}
