//! Filter engine for applying Taiga-style filter rules to RSS items
//!
//! This module provides the logic for filtering and scoring torrent releases
//! based on configurable filter rules.

use crate::db::{FilterAction, FilterRule, FilterType, ShowFilterOverride};
use crate::scraper::rss::RssItem;

/// Result of applying filters to an RSS item
#[derive(Debug, Clone)]
pub struct FilterResult {
    pub item: RssItem,
    pub score: i32,
    pub matched_rules: Vec<String>,
}

/// Filter engine that applies rules to RSS items
pub struct FilterEngine {
    rules: Vec<FilterRule>,
    show_overrides: Vec<ShowFilterOverride>,
}

impl FilterEngine {
    /// Create a new filter engine with global rules and optional show-specific overrides
    pub fn new(global_rules: Vec<FilterRule>, show_overrides: Vec<ShowFilterOverride>) -> Self {
        // Sort rules by priority (highest first)
        let mut rules = global_rules;
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        Self {
            rules,
            show_overrides,
        }
    }

    /// Create a filter engine with only global rules
    pub fn with_global_rules(global_rules: Vec<FilterRule>) -> Self {
        Self::new(global_rules, Vec::new())
    }

    /// Apply filters to a list of RSS items
    ///
    /// Returns a list of items that pass all filters, sorted by score (highest first).
    /// Items that match exclude rules or fail require rules are removed.
    pub fn apply(&self, items: Vec<RssItem>) -> Vec<FilterResult> {
        let mut results: Vec<FilterResult> = items
            .into_iter()
            .filter_map(|item| self.evaluate_item(item))
            .collect();

        // Sort by score (highest first), then by seeders
        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| b.item.seeders.cmp(&a.item.seeders))
        });

        results
    }

    /// Evaluate a single item against all rules
    fn evaluate_item(&self, item: RssItem) -> Option<FilterResult> {
        let mut score = 0i32;
        let mut matched_rules = Vec::new();

        // Check if any show override disables a global rule
        let disabled_rule_ids: Vec<u32> = self
            .show_overrides
            .iter()
            .filter(|o| !o.enabled && o.filter_rule_id.is_some())
            .filter_map(|o| o.filter_rule_id)
            .collect();

        // Process global rules
        for rule in &self.rules {
            // Skip disabled rules
            if !rule.enabled {
                continue;
            }

            // Skip rules that are disabled by show overrides
            if disabled_rule_ids.contains(&rule.id) {
                continue;
            }

            if let Some(result) = self.match_rule(rule, &item) {
                match result {
                    MatchResult::Exclude(reason) => {
                        tracing::debug!(
                            "Item '{}' excluded by rule '{}': {}",
                            item.title,
                            rule.name,
                            reason
                        );
                        return None;
                    }
                    MatchResult::RequireFailed(reason) => {
                        tracing::debug!(
                            "Item '{}' failed require rule '{}': {}",
                            item.title,
                            rule.name,
                            reason
                        );
                        return None;
                    }
                    MatchResult::Prefer(points) => {
                        score += points;
                        matched_rules.push(format!("{} (+{})", rule.name, points));
                    }
                    MatchResult::RequirePass => {
                        // Passed a require check, no points but item is allowed
                        matched_rules.push(format!("{} (required)", rule.name));
                    }
                }
            }
        }

        // Process show-specific custom filters (not overrides of global rules)
        for override_rule in &self.show_overrides {
            if override_rule.filter_rule_id.is_some() {
                // This is an override of a global rule, already handled
                continue;
            }

            if !override_rule.enabled {
                continue;
            }

            // This is a show-specific custom filter
            if let (Some(filter_type), Some(pattern)) =
                (&override_rule.filter_type, &override_rule.pattern)
            {
                let matches = self.pattern_matches(*filter_type, pattern, &item);
                match override_rule.action {
                    FilterAction::Exclude if matches => {
                        tracing::debug!(
                            "Item '{}' excluded by show filter: {} = {}",
                            item.title,
                            filter_type.as_str(),
                            pattern
                        );
                        return None;
                    }
                    FilterAction::Require if !matches => {
                        tracing::debug!(
                            "Item '{}' failed show require filter: {} = {}",
                            item.title,
                            filter_type.as_str(),
                            pattern
                        );
                        return None;
                    }
                    FilterAction::Prefer if matches => {
                        // Show-specific prefer rules get lower priority
                        score += 5;
                        matched_rules.push(format!("show:{} (+5)", pattern));
                    }
                    _ => {}
                }
            }
        }

        Some(FilterResult {
            item,
            score,
            matched_rules,
        })
    }

    /// Match a rule against an item
    fn match_rule(&self, rule: &FilterRule, item: &RssItem) -> Option<MatchResult> {
        let matches = self.pattern_matches(rule.filter_type, &rule.pattern, item);

        match rule.action {
            FilterAction::Exclude => {
                if matches {
                    Some(MatchResult::Exclude(format!(
                        "matched exclude pattern '{}'",
                        rule.pattern
                    )))
                } else {
                    None
                }
            }
            FilterAction::Require => {
                if matches {
                    Some(MatchResult::RequirePass)
                } else {
                    Some(MatchResult::RequireFailed(format!(
                        "did not match required pattern '{}'",
                        rule.pattern
                    )))
                }
            }
            FilterAction::Prefer => {
                if matches {
                    // Score based on priority
                    let points = rule.priority.max(1);
                    Some(MatchResult::Prefer(points))
                } else {
                    None
                }
            }
        }
    }

    /// Check if a pattern matches an item based on filter type
    fn pattern_matches(&self, filter_type: FilterType, pattern: &str, item: &RssItem) -> bool {
        let title_lower = item.title.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        match filter_type {
            FilterType::Resolution => {
                // Match resolution patterns like "1080p", "720p", etc.
                title_lower.contains(&pattern_lower)
            }
            FilterType::Group => {
                // Match fansub group names (usually in brackets at the start)
                // Pattern: [GroupName] at the start of the title
                let group_pattern = format!("[{}]", pattern_lower);
                title_lower.contains(&group_pattern)
                    || title_lower.starts_with(&format!("[{}]", pattern_lower))
            }
            FilterType::TitleExclude | FilterType::TitleInclude => {
                // Case-insensitive substring match
                title_lower.contains(&pattern_lower)
            }
        }
    }
}

/// Internal result type for rule matching
enum MatchResult {
    Exclude(String),
    RequireFailed(String),
    RequirePass,
    Prefer(i32),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rss_item(title: &str) -> RssItem {
        RssItem {
            title: title.to_string(),
            torrent_link: "https://example.com/test.torrent".to_string(),
            view_url: "https://example.com/view/1".to_string(),
            pub_date: "2026-01-01".to_string(),
            info_hash: "abc123".to_string(),
            category_id: "1_2".to_string(),
            size: "1 GiB".to_string(),
            seeders: 10,
            leechers: 5,
        }
    }

    fn make_filter(
        id: u32,
        name: &str,
        filter_type: FilterType,
        pattern: &str,
        action: FilterAction,
        priority: i32,
    ) -> FilterRule {
        FilterRule {
            id,
            name: name.to_string(),
            filter_type,
            pattern: pattern.to_string(),
            action,
            priority,
            is_global: true,
            enabled: true,
            created_at: None,
        }
    }

    #[test]
    fn test_exclude_filter() {
        let rules = vec![make_filter(
            1,
            "Exclude batches",
            FilterType::TitleExclude,
            "batch",
            FilterAction::Exclude,
            100,
        )];

        let engine = FilterEngine::with_global_rules(rules);

        let items = vec![
            make_rss_item("[SubsPlease] One Piece - 1060 (1080p).mkv"),
            make_rss_item("[SubsPlease] One Piece - batch (1080p).mkv"),
        ];

        let results = engine.apply(items);

        assert_eq!(results.len(), 1);
        assert!(results[0].item.title.contains("1060"));
    }

    #[test]
    fn test_prefer_filter() {
        let rules = vec![make_filter(
            1,
            "Prefer 1080p",
            FilterType::Resolution,
            "1080p",
            FilterAction::Prefer,
            10,
        )];

        let engine = FilterEngine::with_global_rules(rules);

        let items = vec![
            make_rss_item("[SubsPlease] One Piece - 1060 (720p).mkv"),
            make_rss_item("[SubsPlease] One Piece - 1060 (1080p).mkv"),
        ];

        let results = engine.apply(items);

        // Both should pass, but 1080p should have higher score
        assert_eq!(results.len(), 2);
        assert!(results[0].item.title.contains("1080p"));
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_require_filter() {
        let rules = vec![make_filter(
            1,
            "Require SubsPlease",
            FilterType::Group,
            "SubsPlease",
            FilterAction::Require,
            50,
        )];

        let engine = FilterEngine::with_global_rules(rules);

        let items = vec![
            make_rss_item("[SubsPlease] One Piece - 1060 (1080p).mkv"),
            make_rss_item("[Erai-raws] One Piece - 1060 (1080p).mkv"),
        ];

        let results = engine.apply(items);

        // Only SubsPlease should pass
        assert_eq!(results.len(), 1);
        assert!(results[0].item.title.contains("SubsPlease"));
    }

    #[test]
    fn test_combined_filters() {
        let rules = vec![
            make_filter(
                1,
                "Exclude batches",
                FilterType::TitleExclude,
                "batch",
                FilterAction::Exclude,
                100,
            ),
            make_filter(
                2,
                "Prefer 1080p",
                FilterType::Resolution,
                "1080p",
                FilterAction::Prefer,
                10,
            ),
            make_filter(
                3,
                "Prefer SubsPlease",
                FilterType::Group,
                "SubsPlease",
                FilterAction::Prefer,
                5,
            ),
        ];

        let engine = FilterEngine::with_global_rules(rules);

        let items = vec![
            make_rss_item("[SubsPlease] One Piece - 1060 (1080p).mkv"),
            make_rss_item("[Erai-raws] One Piece - 1060 (1080p).mkv"),
            make_rss_item("[SubsPlease] One Piece - 1060 (720p).mkv"),
            make_rss_item("[SubsPlease] One Piece - batch (1080p).mkv"),
        ];

        let results = engine.apply(items);

        // Batch should be excluded, rest should be sorted by score
        assert_eq!(results.len(), 3);

        // First should be SubsPlease + 1080p (highest score)
        assert!(results[0].item.title.contains("SubsPlease"));
        assert!(results[0].item.title.contains("1080p"));
        assert!(results[0].score == 15); // 10 + 5
    }

    #[test]
    fn test_disabled_filter() {
        let mut rules = vec![make_filter(
            1,
            "Exclude batches",
            FilterType::TitleExclude,
            "batch",
            FilterAction::Exclude,
            100,
        )];

        // Disable the filter
        rules[0].enabled = false;

        let engine = FilterEngine::with_global_rules(rules);

        let items = vec![make_rss_item("[SubsPlease] One Piece - batch (1080p).mkv")];

        let results = engine.apply(items);

        // Batch should NOT be excluded because filter is disabled
        assert_eq!(results.len(), 1);
    }
}
