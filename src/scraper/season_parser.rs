//! Anime season number detection from titles
//!
//! Implements robust parsing for various season naming conventions used in anime:
//! - S1, S2, S01, S02 (prefix notation)
//! - Season 1, Season 2, Saison 2 (spelled out)
//! - 2nd Season, 3rd Season (ordinal format)
//! - Part 2, Part II (part notation)
//! - Cour 2 (anime-specific terminology)
//! - II, III, IV (Roman numerals)
//! - Title 2, Title: 2 (trailing numbers)
//!
//! Based on patterns from Sonarr, Anitomy, and common fansub conventions.

use regex::Regex;

/// Result of season parsing
#[derive(Debug, Clone, PartialEq)]
pub struct SeasonInfo {
    /// Detected season number (1 if not detected)
    pub season: u8,
    /// The pattern that matched (for debugging)
    pub matched_pattern: Option<String>,
    /// Cleaned title with season indicator removed (optional)
    pub clean_title: Option<String>,
}

impl Default for SeasonInfo {
    fn default() -> Self {
        Self {
            season: 1,
            matched_pattern: None,
            clean_title: None,
        }
    }
}

/// Detects the season number from an anime title
///
/// # Arguments
/// * `title` - The anime title to parse
///
/// # Returns
/// A `SeasonInfo` struct containing the detected season and metadata
///
/// # Examples
/// ```ignore
/// let info = detect_season("Sousou no Frieren S2");
/// assert_eq!(info.season, 2);
///
/// let info = detect_season("My Hero Academia 2nd Season");
/// assert_eq!(info.season, 2);
/// ```
pub fn detect_season(title: &str) -> SeasonInfo {
    // Try each pattern in order of specificity

    // Pattern 1: S## format (S1, S2, S01, S02, etc.)
    // Matches: "Title S2", "Title S02", "[Group] Title S2"
    if let Some(info) = detect_s_prefix(title) {
        return info;
    }

    // Pattern 2: "Season ##" or "Saison ##" (spelled out)
    // Matches: "Title Season 2", "Title: Season 2"
    if let Some(info) = detect_spelled_season(title) {
        return info;
    }

    // Pattern 3: Ordinal format (2nd Season, 3rd Season, etc.)
    // Matches: "Title 2nd Season", "Title: 3rd Season"
    if let Some(info) = detect_ordinal_season(title) {
        return info;
    }

    // Pattern 4: Part notation (Part 2, Part II)
    // Matches: "Title Part 2", "Title: Part II"
    if let Some(info) = detect_part(title) {
        return info;
    }

    // Pattern 5: Cour notation (Cour 2)
    // Matches: "Title Cour 2", "Title: Cour 2"
    if let Some(info) = detect_cour(title) {
        return info;
    }

    // Pattern 6: Roman numerals at end (II, III, IV, etc.)
    // Matches: "Title II", "Title III"
    if let Some(info) = detect_roman_numeral(title) {
        return info;
    }

    // Pattern 7: Trailing number (Title 2, Title: 2)
    // Matches: "Oregairu 2", "SAO 3"
    // This is the least specific, so we're more careful
    if let Some(info) = detect_trailing_number(title) {
        return info;
    }

    // No season detected, default to 1
    SeasonInfo::default()
}

/// Detects S## prefix format
fn detect_s_prefix(title: &str) -> Option<SeasonInfo> {
    // Case insensitive match for S followed by 1-2 digits
    // Negative lookbehind to avoid matching in the middle of words
    let re = Regex::new(r"(?i)\bS(\d{1,2})\b").ok()?;

    if let Some(caps) = re.captures(title) {
        let season_str = caps.get(1)?.as_str();
        let season: u8 = season_str.parse().ok()?;

        // Clean the title by removing the season marker
        let clean = re.replace(title, "").trim().to_string();

        return Some(SeasonInfo {
            season,
            matched_pattern: Some(format!("S{}", season_str)),
            clean_title: Some(clean),
        });
    }

    None
}

/// Detects spelled out "Season ##" or "Saison ##"
fn detect_spelled_season(title: &str) -> Option<SeasonInfo> {
    // Matches Season, Saison, Series, Stagione (Italian)
    let re = Regex::new(r"(?i)\b(Season|Saison|Series|Stagione)[-_.\s]?(\d{1,2})\b").ok()?;

    if let Some(caps) = re.captures(title) {
        let matched = caps.get(0)?.as_str();
        let season: u8 = caps.get(2)?.as_str().parse().ok()?;

        let clean = title.replace(matched, "").trim().to_string();

        return Some(SeasonInfo {
            season,
            matched_pattern: Some(matched.to_string()),
            clean_title: Some(clean),
        });
    }

    None
}

/// Detects ordinal format (2nd Season, 3rd Season)
fn detect_ordinal_season(title: &str) -> Option<SeasonInfo> {
    // Matches: 1st, 2nd, 3rd, 4th, etc. followed by Season
    let re = Regex::new(r"(?i)\b(\d{1,2})(st|nd|rd|th)\s*(Season|Cour)?\b").ok()?;

    if let Some(caps) = re.captures(title) {
        let matched = caps.get(0)?.as_str();
        let season: u8 = caps.get(1)?.as_str().parse().ok()?;

        let clean = title.replace(matched, "").trim().to_string();

        return Some(SeasonInfo {
            season,
            matched_pattern: Some(matched.to_string()),
            clean_title: Some(clean),
        });
    }

    None
}

/// Detects Part notation
fn detect_part(title: &str) -> Option<SeasonInfo> {
    // First try Part + Arabic numeral
    let re_arabic = Regex::new(r"(?i)\bPart[-_.\s]?(\d{1,2})\b").ok()?;

    if let Some(caps) = re_arabic.captures(title) {
        let matched = caps.get(0)?.as_str();
        let season: u8 = caps.get(1)?.as_str().parse().ok()?;

        let clean = title.replace(matched, "").trim().to_string();

        return Some(SeasonInfo {
            season,
            matched_pattern: Some(matched.to_string()),
            clean_title: Some(clean),
        });
    }

    // Try Part + Roman numeral
    let re_roman = Regex::new(r"(?i)\bPart[-_.\s]?(I{1,3}|IV|VI{0,3}|IX|X)\b").ok()?;

    if let Some(caps) = re_roman.captures(title) {
        let matched = caps.get(0)?.as_str();
        let roman = caps.get(1)?.as_str();
        let season = roman_to_arabic(roman)?;

        let clean = title.replace(matched, "").trim().to_string();

        return Some(SeasonInfo {
            season,
            matched_pattern: Some(matched.to_string()),
            clean_title: Some(clean),
        });
    }

    None
}

/// Detects Cour notation (anime-specific)
fn detect_cour(title: &str) -> Option<SeasonInfo> {
    let re = Regex::new(r"(?i)\bCour[-_.\s]?(\d{1,2})\b").ok()?;

    if let Some(caps) = re.captures(title) {
        let matched = caps.get(0)?.as_str();
        let season: u8 = caps.get(1)?.as_str().parse().ok()?;

        let clean = title.replace(matched, "").trim().to_string();

        return Some(SeasonInfo {
            season,
            matched_pattern: Some(matched.to_string()),
            clean_title: Some(clean),
        });
    }

    None
}

/// Detects Roman numerals at end of title
fn detect_roman_numeral(title: &str) -> Option<SeasonInfo> {
    // Only match at word boundary, typically end of title
    // Matches II, III, IV, V, VI, VII, VIII, IX, X
    let re = Regex::new(r"\b(X{0,1}(?:IX|IV|V?I{1,3}))\s*$").ok()?;

    if let Some(caps) = re.captures(title) {
        let matched = caps.get(1)?.as_str();
        let season = roman_to_arabic(matched)?;

        // Only consider valid if season > 1 (single "I" is often part of title)
        if season > 1 {
            let clean = re.replace(title, "").trim().to_string();

            return Some(SeasonInfo {
                season,
                matched_pattern: Some(matched.to_string()),
                clean_title: Some(clean),
            });
        }
    }

    None
}

/// Detects trailing number (least specific pattern)
fn detect_trailing_number(title: &str) -> Option<SeasonInfo> {
    // Match number at end, with optional colon, dash, or space separator
    // Be careful not to match years (4 digits) or episode numbers
    // Patterns: "Title 2", "Title: 2", "Title - 2"
    let re = Regex::new(r"(?:[-:]\s*|\s+)(\d)\s*$").ok()?;

    if let Some(caps) = re.captures(title) {
        let season_str = caps.get(1)?.as_str();
        let season: u8 = season_str.parse().ok()?;

        // Sanity check: season should be reasonable (1-9, single digit only for trailing)
        if season >= 1 && season <= 9 {
            let matched = caps.get(0)?.as_str();
            let clean = title.replace(matched, "").trim().to_string();

            return Some(SeasonInfo {
                season,
                matched_pattern: Some(format!("trailing {}", season)),
                clean_title: Some(clean),
            });
        }
    }

    None
}

/// Converts Roman numeral to Arabic number
fn roman_to_arabic(roman: &str) -> Option<u8> {
    let roman = roman.to_uppercase();

    match roman.as_str() {
        "I" => Some(1),
        "II" => Some(2),
        "III" => Some(3),
        "IV" => Some(4),
        "V" => Some(5),
        "VI" => Some(6),
        "VII" => Some(7),
        "VIII" => Some(8),
        "IX" => Some(9),
        "X" => Some(10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s_prefix() {
        assert_eq!(detect_season("Sousou no Frieren S2").season, 2);
        assert_eq!(detect_season("One Piece S01").season, 1);
        assert_eq!(detect_season("[SubsPlease] Title S2 - 01").season, 2);
        assert_eq!(detect_season("Attack on Titan S4").season, 4);
    }

    #[test]
    fn test_spelled_season() {
        assert_eq!(detect_season("My Hero Academia Season 2").season, 2);
        assert_eq!(detect_season("Demon Slayer: Season 3").season, 3);
        assert_eq!(detect_season("Title Saison 2").season, 2);
    }

    #[test]
    fn test_ordinal() {
        assert_eq!(detect_season("My Hero Academia 2nd Season").season, 2);
        assert_eq!(detect_season("Overlord 3rd Season").season, 3);
        assert_eq!(detect_season("Re:Zero 2nd Season").season, 2);
        assert_eq!(detect_season("Title 4th Cour").season, 4);
    }

    #[test]
    fn test_part() {
        assert_eq!(detect_season("Attack on Titan Part 2").season, 2);
        assert_eq!(detect_season("JoJo Part 5").season, 5);
        assert_eq!(detect_season("Title Part II").season, 2);
        assert_eq!(detect_season("Title Part III").season, 3);
    }

    #[test]
    fn test_cour() {
        assert_eq!(detect_season("86 Cour 2").season, 2);
        assert_eq!(detect_season("Title: Cour 2").season, 2);
    }

    #[test]
    fn test_roman_numeral() {
        assert_eq!(detect_season("Spice and Wolf II").season, 2);
        assert_eq!(detect_season("Oregairu III").season, 3);
        assert_eq!(detect_season("Shakugan no Shana III").season, 3);
    }

    #[test]
    fn test_trailing_number() {
        assert_eq!(detect_season("Oregairu 2").season, 2);
        assert_eq!(detect_season("SAO: 3").season, 3);
        assert_eq!(detect_season("Re:Zero - 2").season, 2);
    }

    #[test]
    fn test_no_season() {
        assert_eq!(detect_season("One Punch Man").season, 1);
        assert_eq!(detect_season("Frieren: Beyond Journey's End").season, 1);
        assert_eq!(detect_season("Bocchi the Rock!").season, 1);
    }

    #[test]
    fn test_clean_title() {
        let info = detect_season("Sousou no Frieren S2");
        assert_eq!(info.clean_title, Some("Sousou no Frieren".to_string()));

        let info = detect_season("My Hero Academia 2nd Season");
        assert_eq!(info.clean_title, Some("My Hero Academia".to_string()));
    }

    #[test]
    fn test_edge_cases() {
        // Year shouldn't be confused with season
        assert_eq!(detect_season("Title (2024)").season, 1);

        // S in the middle of title shouldn't match
        assert_eq!(detect_season("Monster Strike").season, 1);

        // Double digit numbers shouldn't match trailing pattern (likely episode)
        assert_eq!(detect_season("Title - 12").season, 1); // 12 is likely episode

        // Single digit at end should match
        assert_eq!(detect_season("Title 3").season, 3);
    }
}
