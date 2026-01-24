//! RSS feed parser for Nyaa.si and SubsPlease
//!
//! This module provides functionality to fetch and parse RSS feeds from nyaa.si
//! and subsplease.org, extracting torrent information including episode details,
//! quality, and magnet links.

use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// RSS source type for fetching torrents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RssSource {
    /// Nyaa.si RSS - searches with source + title query
    Nyaa,
    /// SubsPlease direct RSS - fetches all releases at specified quality
    SubsPleaseDirect,
}

impl RssSource {
    pub fn from_source_string(source: &str) -> Self {
        match source.to_lowercase().as_str() {
            "subsplease_direct" => RssSource::SubsPleaseDirect,
            _ => RssSource::Nyaa,
        }
    }
}

/// Normalizes an anime title for RSS search by removing season suffixes
///
/// Nyaa/SubsPlease typically use "S2" format instead of "2nd Season" or "Season 2".
/// This function strips these suffixes so the search will match.
///
/// # Examples
/// - "Sousou no Frieren 2nd Season" -> "Sousou no Frieren"
/// - "My Hero Academia Season 7" -> "My Hero Academia"
/// - "One Piece" -> "One Piece" (unchanged)
fn normalize_title_for_search(title: &str) -> String {
    let patterns = [
        r"\s+(?:2nd|3rd|[4-9]th)\s+Season\s*$",           // "2nd Season", "3rd Season", etc.
        r"\s+Season\s+\d+\s*$",                           // "Season 2", "Season 10"
        r"\s+S\d+\s*$",                                   // Already has "S2" format
        r"\s+Part\s+\d+\s*$",                             // "Part 2"
        r"\s+(?:II|III|IV|V|VI|VII|VIII|IX|X)\s*$",       // Roman numerals
        r"\s+Cour\s+\d+\s*$",                             // "Cour 2"
    ];

    let mut result = title.to_string();
    for pattern in patterns {
        if let Ok(re) = Regex::new(&format!("(?i){}", pattern)) {
            result = re.replace(&result, "").to_string();
        }
    }
    result.trim().to_string()
}

/// Represents a single item from the Nyaa RSS feed
#[derive(Debug, Clone)]
pub struct RssItem {
    pub title: String,
    pub torrent_link: String,   // Direct .torrent download URL
    pub view_url: String,       // https://nyaa.si/view/ID
    pub pub_date: String,
    pub info_hash: String,
    pub category_id: String,
    pub size: String,
    pub seeders: u32,
    pub leechers: u32,
}

/// Represents a parsed episode with extracted metadata
#[derive(Debug, Clone)]
pub struct ParsedEpisode {
    pub show_title: String,
    pub episode: u16,
    pub quality: String,        // 480p, 720p, 1080p, 2160p
    pub info_hash: String,
    pub torrent_url: String,
    pub magnet_url: String,     // Constructed from info_hash
}

/// Fetches and parses an RSS feed from nyaa.si
///
/// # Arguments
/// * `source` - The uploader name (e.g., "subsplease", "Erai-raws")
/// * `alternate` - The search term / show name
///
/// # Returns
/// A vector of `RssItem` parsed from the feed
///
/// # Example
/// ```ignore
/// let items = fetch_rss_feed("subsplease", "One Piece").await?;
/// ```
pub async fn fetch_rss_feed(source: &str, alternate: &str) -> Result<Vec<RssItem>> {
    // Normalize title to remove season suffixes that don't match Nyaa naming
    let normalized_title = normalize_title_for_search(alternate);
    let query = format!("{} {}", source, normalized_title);
    let encoded_query = urlencoding::encode(&query);
    let url = format!(
        "https://nyaa.si/?page=rss&q={}&c=1_2&f=0",
        encoded_query
    );

    tracing::debug!("Fetching Nyaa RSS: {}", url);
    if normalized_title != alternate {
        tracing::debug!("Title normalized: '{}' -> '{}'", alternate, normalized_title);
    }

    let response = super::http_client()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch RSS feed from {}", url))?;

    let xml = response
        .text()
        .await
        .with_context(|| "Failed to read response body")?;

    parse_rss_xml(&xml)
}

/// Fetches RSS feed from SubsPlease.org
///
/// SubsPlease provides a direct RSS feed at `subsplease.org/rss/?t&r={quality}`
/// that contains all their releases at the specified quality.
///
/// # Arguments
/// * `quality` - Quality filter: "1080", "720", or "480" (without 'p' suffix)
///
/// # Returns
/// A vector of `RssItem` parsed from the feed
///
/// # Example
/// ```ignore
/// let items = fetch_subsplease_rss("1080").await?;
/// ```
pub async fn fetch_subsplease_rss(quality: &str) -> Result<Vec<RssItem>> {
    // SubsPlease uses quality without 'p' suffix in their RSS URL
    let quality_param = quality.trim_end_matches('p');
    let url = format!("https://subsplease.org/rss/?t&r={}", quality_param);

    let response = super::http_client()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch SubsPlease RSS feed from {}", url))?;

    let xml = response
        .text()
        .await
        .with_context(|| "Failed to read SubsPlease response body")?;

    parse_subsplease_rss_xml(&xml)
}

/// Fetches RSS items using the appropriate source
///
/// # Arguments
/// * `source` - The RSS source type (Nyaa or SubsPleaseDirect)
/// * `source_name` - For Nyaa: the uploader name; for SubsPlease: ignored
/// * `show_name` - The show name to search/filter for
/// * `quality` - Quality preference (e.g., "1080p")
///
/// # Returns
/// A vector of `RssItem` matching the criteria
pub async fn fetch_rss_by_source(
    source: RssSource,
    source_name: &str,
    show_name: &str,
    quality: &str,
) -> Result<Vec<RssItem>> {
    match source {
        RssSource::Nyaa => {
            // Nyaa: search with source + show name
            fetch_rss_feed(source_name, show_name).await
        }
        RssSource::SubsPleaseDirect => {
            // SubsPlease: fetch all at quality, then filter by show name
            let all_items = fetch_subsplease_rss(quality).await?;

            // Filter items that match the show name (case-insensitive)
            let show_lower = show_name.to_lowercase();
            let filtered: Vec<RssItem> = all_items
                .into_iter()
                .filter(|item| {
                    let title_lower = item.title.to_lowercase();
                    // Check if the show name appears in the title
                    title_lower.contains(&show_lower)
                })
                .collect();

            Ok(filtered)
        }
    }
}

/// Parses SubsPlease RSS XML content into a vector of RssItem
///
/// SubsPlease RSS has a simpler format than Nyaa - it doesn't include
/// nyaa: namespaced elements. The link points to a nyaa.si page.
///
/// # Arguments
/// * `xml` - The raw XML string from the SubsPlease RSS feed
///
/// # Returns
/// A vector of parsed `RssItem` structs
pub fn parse_subsplease_rss_xml(xml: &str) -> Result<Vec<RssItem>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut items = Vec::new();
    let mut buf = Vec::new();

    let mut current_item: Option<SubsPleaseItemBuilder> = None;
    let mut current_element: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match name.as_str() {
                    "item" => {
                        current_item = Some(SubsPleaseItemBuilder::default());
                    }
                    _ => {
                        current_element = Some(name);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "item" {
                    if let Some(builder) = current_item.take() {
                        if let Some(item) = builder.build() {
                            items.push(item);
                        }
                    }
                }
                current_element = None;
            }
            Ok(Event::Text(ref e)) => {
                if let (Some(item), Some(element)) = (&mut current_item, &current_element) {
                    let text = e.unescape().unwrap_or_default().to_string();
                    if !text.is_empty() {
                        match element.as_str() {
                            "title" => item.title = Some(text),
                            "link" => item.link = Some(text),
                            "guid" => item.guid = Some(text),
                            "pubDate" => item.pub_date = Some(text),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Error parsing SubsPlease XML at position {}: {:?}",
                    reader.buffer_position(),
                    e
                ));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}

/// Builder for SubsPlease RSS items
#[derive(Default)]
struct SubsPleaseItemBuilder {
    title: Option<String>,
    link: Option<String>,
    guid: Option<String>,
    pub_date: Option<String>,
}

impl SubsPleaseItemBuilder {
    fn build(self) -> Option<RssItem> {
        let title = self.title?;
        let link = self.link.unwrap_or_default();

        // SubsPlease links point to nyaa.si, extract torrent URL
        // Link format: https://nyaa.si/view/ID
        let (torrent_link, view_url, info_hash) = if link.contains("nyaa.si/view/") {
            // Extract ID and construct torrent URL
            let id = link.split('/').last().unwrap_or("");
            let torrent_url = format!("https://nyaa.si/download/{}.torrent", id);
            (torrent_url, link.clone(), String::new())
        } else {
            // Fallback: use the link as-is
            (link.clone(), link, String::new())
        };

        Some(RssItem {
            title,
            torrent_link,
            view_url,
            pub_date: self.pub_date.unwrap_or_default(),
            info_hash,
            category_id: "1_2".to_string(), // Anime - English-translated
            size: String::new(),
            seeders: 0,
            leechers: 0,
        })
    }
}

/// Parses RSS XML content into a vector of RssItem
///
/// Handles the nyaa: namespace prefix for custom elements like seeders, leechers, etc.
///
/// # Arguments
/// * `xml` - The raw XML string from the RSS feed
///
/// # Returns
/// A vector of parsed `RssItem` structs
pub fn parse_rss_xml(xml: &str) -> Result<Vec<RssItem>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut items = Vec::new();
    let mut buf = Vec::new();

    // Current item being parsed
    let mut current_item: Option<RssItemBuilder> = None;
    let mut current_element: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match name.as_str() {
                    "item" => {
                        current_item = Some(RssItemBuilder::default());
                    }
                    _ => {
                        current_element = Some(name);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "item" {
                    if let Some(builder) = current_item.take() {
                        if let Some(item) = builder.build() {
                            items.push(item);
                        }
                    }
                }
                current_element = None;
            }
            Ok(Event::Text(ref e)) => {
                if let (Some(item), Some(element)) = (&mut current_item, &current_element) {
                    let text = e.unescape().unwrap_or_default().to_string();
                    if !text.is_empty() {
                        match element.as_str() {
                            "title" => item.title = Some(text),
                            "link" => item.torrent_link = Some(text),
                            "guid" => item.view_url = Some(text),
                            "pubDate" => item.pub_date = Some(text),
                            "nyaa:seeders" => {
                                item.seeders = text.parse().ok();
                            }
                            "nyaa:leechers" => {
                                item.leechers = text.parse().ok();
                            }
                            "nyaa:infoHash" => item.info_hash = Some(text),
                            "nyaa:categoryId" => item.category_id = Some(text),
                            "nyaa:size" => item.size = Some(text),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Error parsing XML at position {}: {:?}",
                    reader.buffer_position(),
                    e
                ));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}

/// Builder pattern for constructing RssItem during XML parsing
#[derive(Default)]
struct RssItemBuilder {
    title: Option<String>,
    torrent_link: Option<String>,
    view_url: Option<String>,
    pub_date: Option<String>,
    info_hash: Option<String>,
    category_id: Option<String>,
    size: Option<String>,
    seeders: Option<u32>,
    leechers: Option<u32>,
}

impl RssItemBuilder {
    fn build(self) -> Option<RssItem> {
        Some(RssItem {
            title: self.title?,
            torrent_link: self.torrent_link.unwrap_or_default(),
            view_url: self.view_url.unwrap_or_default(),
            pub_date: self.pub_date.unwrap_or_default(),
            info_hash: self.info_hash.unwrap_or_default(),
            category_id: self.category_id.unwrap_or_default(),
            size: self.size.unwrap_or_default(),
            seeders: self.seeders.unwrap_or(0),
            leechers: self.leechers.unwrap_or(0),
        })
    }
}

/// Parses episode information from a torrent title
///
/// Extracts the show name, episode number, and quality from typical anime release titles.
///
/// # Arguments
/// * `title` - The torrent title string
///
/// # Returns
/// A tuple of (show_title, episode_number, quality) if parsing succeeds
/// Parsed episode information including optional season
#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeInfo {
    pub show_title: String,
    pub season: Option<u16>,
    pub episode: u16,
    pub quality: String,
}

///
/// # Examples
/// ```ignore
/// let info = parse_episode_info("[SubsPlease] One Piece - 1060 (1080p) [37A98D45].mkv");
/// assert_eq!(info, Some(("One Piece".to_string(), 1060, "1080p".to_string())));
/// ```
pub fn parse_episode_info(title: &str) -> Option<(String, u16, String)> {
    let info = parse_episode_info_full(title)?;
    Some((info.show_title, info.episode, info.quality))
}

/// Parses episode information including season number from a release title
///
/// Handles formats like:
/// - `[SubsPlease] Show Name S2 - 02 (1080p)` -> season 2, episode 2
/// - `[Erai-raws] Show Name 3rd Season - 01 [1080p]` -> season 3, episode 1
/// - `[SubsPlease] Show Name - 28 (1080p)` -> season 1 (default), episode 28
pub fn parse_episode_info_full(title: &str) -> Option<EpisodeInfo> {
    // Pattern with explicit season: [Source] Show Name S2 - Episode (Quality)
    let re_with_season = Regex::new(r"\[.*?\]\s*(.*?)\s+S(\d+)\s*-\s*(\d+)\s*.*?(\d{3,4}p)").ok()?;

    if let Some(captures) = re_with_season.captures(title) {
        let show_title = captures.get(1)?.as_str().trim().to_string();
        let season: u16 = captures.get(2)?.as_str().parse().ok()?;
        let episode: u16 = captures.get(3)?.as_str().parse().ok()?;
        let quality = captures.get(4)?.as_str().to_string();

        return Some(EpisodeInfo {
            show_title,
            season: Some(season),
            episode,
            quality,
        });
    }

    // Pattern with ordinal season: [Source] Show Name 2nd Season - Episode [Quality]
    // Matches: "2nd Season", "3rd Season", "4th Season", etc.
    let re_ordinal_season =
        Regex::new(r"\[.*?\]\s*(.*?)\s+(\d+)(?:st|nd|rd|th)\s+Season\s*-\s*(\d+)\s*.*?(\d{3,4}p)")
            .ok()?;

    if let Some(captures) = re_ordinal_season.captures(title) {
        let show_title = captures.get(1)?.as_str().trim().to_string();
        let season: u16 = captures.get(2)?.as_str().parse().ok()?;
        let episode: u16 = captures.get(3)?.as_str().parse().ok()?;
        let quality = captures.get(4)?.as_str().to_string();

        return Some(EpisodeInfo {
            show_title,
            season: Some(season),
            episode,
            quality,
        });
    }

    // Pattern with "Season N": [Source] Show Name Season 2 - Episode [Quality]
    let re_season_n =
        Regex::new(r"\[.*?\]\s*(.*?)\s+Season\s+(\d+)\s*-\s*(\d+)\s*.*?(\d{3,4}p)").ok()?;

    if let Some(captures) = re_season_n.captures(title) {
        let show_title = captures.get(1)?.as_str().trim().to_string();
        let season: u16 = captures.get(2)?.as_str().parse().ok()?;
        let episode: u16 = captures.get(3)?.as_str().parse().ok()?;
        let quality = captures.get(4)?.as_str().to_string();

        return Some(EpisodeInfo {
            show_title,
            season: Some(season),
            episode,
            quality,
        });
    }

    // Pattern without season: [Source] Show Name - Episode (Quality)
    let re_no_season = Regex::new(r"\[.*?\]\s*(.*?)\s*-\s*(\d+)\s*.*?(\d{3,4}p)").ok()?;

    if let Some(captures) = re_no_season.captures(title) {
        let show_title = captures.get(1)?.as_str().trim().to_string();
        let episode: u16 = captures.get(2)?.as_str().parse().ok()?;
        let quality = captures.get(3)?.as_str().to_string();

        return Some(EpisodeInfo {
            show_title,
            season: None, // Could be season 1 or a long-running show
            episode,
            quality,
        });
    }

    None
}

/// Detects the fansub source/group from a torrent title
///
/// Parses common fansub group brackets from anime release titles.
///
/// # Arguments
/// * `title` - The torrent title string (e.g., "[SubsPlease] One Piece - 1060 (1080p)")
///
/// # Returns
/// The detected source name (lowercase), or "subsplease" as default
///
/// # Examples
/// ```ignore
/// assert_eq!(detect_fansub_source("[SubsPlease] One Piece - 01 (1080p)"), "subsplease");
/// assert_eq!(detect_fansub_source("[Erai-raws] Frieren - 01 [1080p]"), "Erai-raws");
/// assert_eq!(detect_fansub_source("[Judas] Attack on Titan - 01.mkv"), "judas");
/// ```
pub fn detect_fansub_source(title: &str) -> String {
    // Pattern to extract group name from brackets at the start
    let re = Regex::new(r"^\[([^\]]+)\]").ok();

    if let Some(regex) = re {
        if let Some(captures) = regex.captures(title) {
            if let Some(group) = captures.get(1) {
                let source = group.as_str().trim();
                // Return common groups with their canonical casing
                return match source.to_lowercase().as_str() {
                    "subsplease" => "subsplease".to_string(),
                    "erai-raws" => "Erai-raws".to_string(),
                    "horriblesubs" => "horriblesubs".to_string(),
                    "judas" => "judas".to_string(),
                    "yameii" => "yameii".to_string(),
                    "ember" => "ember".to_string(),
                    "asm" => "asm".to_string(),
                    _ => source.to_string(), // Preserve original casing for unknown groups
                };
            }
        }
    }

    // Default to subsplease if no group found
    "subsplease".to_string()
}

/// Constructs a magnet URL from an info hash and title
///
/// # Arguments
/// * `info_hash` - The torrent info hash (40 character hex string)
/// * `title` - The display name for the torrent
///
/// # Returns
/// A properly formatted magnet URL
///
/// # Example
/// ```ignore
/// let magnet = construct_magnet_url("e30690d4a8d1f5e45f5ded430bdaedc710da0245", "Show Name");
/// assert!(magnet.starts_with("magnet:?xt=urn:btih:"));
/// ```
pub fn construct_magnet_url(info_hash: &str, title: &str) -> String {
    let encoded_title = urlencoding::encode(title);
    format!(
        "magnet:?xt=urn:btih:{}&dn={}",
        info_hash, encoded_title
    )
}

/// Filters RSS items by video quality
///
/// # Arguments
/// * `items` - Slice of RssItem to filter
/// * `quality` - Quality string to match (e.g., "1080p", "720p")
///
/// # Returns
/// A vector of references to items that contain the specified quality in their title
pub fn filter_by_quality<'a>(items: &'a [RssItem], quality: &str) -> Vec<&'a RssItem> {
    items
        .iter()
        .filter(|item| item.title.contains(quality))
        .collect()
}

/// Converts an RssItem to a ParsedEpisode
///
/// Combines RSS data with parsed episode information to create a complete
/// episode representation with magnet URL.
///
/// # Arguments
/// * `item` - The RssItem to convert
///
/// # Returns
/// Some(ParsedEpisode) if the title can be parsed, None otherwise
pub fn rss_item_to_parsed_episode(item: &RssItem) -> Option<ParsedEpisode> {
    let (show_title, episode, quality) = parse_episode_info(&item.title)?;
    let magnet_url = construct_magnet_url(&item.info_hash, &item.title);

    Some(ParsedEpisode {
        show_title,
        episode,
        quality,
        info_hash: item.info_hash.clone(),
        torrent_url: item.torrent_link.clone(),
        magnet_url,
    })
}

/// Fetches and parses episodes from RSS feed, returning only valid parsed episodes
///
/// This is a convenience function that combines fetching, parsing, and filtering
/// into a single operation.
///
/// # Arguments
/// * `source` - The uploader name
/// * `alternate` - The search term / show name
/// * `quality` - Optional quality filter (e.g., "1080p")
///
/// # Returns
/// A vector of successfully parsed episodes
pub async fn fetch_episodes(
    source: &str,
    alternate: &str,
    quality: Option<&str>,
) -> Result<Vec<ParsedEpisode>> {
    let items = fetch_rss_feed(source, alternate).await?;

    let filtered_items: Vec<&RssItem> = if let Some(q) = quality {
        filter_by_quality(&items, q)
    } else {
        items.iter().collect()
    };

    let episodes: Vec<ParsedEpisode> = filtered_items
        .into_iter()
        .filter_map(rss_item_to_parsed_episode)
        .collect();

    Ok(episodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss xmlns:atom="http://www.w3.org/2005/Atom" xmlns:nyaa="https://nyaa.si/xmlns/nyaa" version="2.0">
  <channel>
    <title>Nyaa - Search - Torrent File RSS</title>
    <item>
      <title>[SubsPlease] One Piece - 1060 (1080p) [37A98D45].mkv</title>
      <link>https://nyaa.si/download/2059096.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/2059096</guid>
      <pubDate>Tue, 30 Dec 2025 06:22:52 -0000</pubDate>
      <nyaa:seeders>18</nyaa:seeders>
      <nyaa:leechers>8</nyaa:leechers>
      <nyaa:infoHash>e30690d4a8d1f5e45f5ded430bdaedc710da0245</nyaa:infoHash>
      <nyaa:categoryId>1_2</nyaa:categoryId>
      <nyaa:size>1.2 GiB</nyaa:size>
    </item>
    <item>
      <title>[Erai-raws] Goblin Slayer II - 03 [720p][Multiple Subtitle]</title>
      <link>https://nyaa.si/download/2059097.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/2059097</guid>
      <pubDate>Tue, 30 Dec 2025 05:00:00 -0000</pubDate>
      <nyaa:seeders>50</nyaa:seeders>
      <nyaa:leechers>10</nyaa:leechers>
      <nyaa:infoHash>a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2</nyaa:infoHash>
      <nyaa:categoryId>1_2</nyaa:categoryId>
      <nyaa:size>500 MiB</nyaa:size>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn test_parse_rss_xml() {
        let items = parse_rss_xml(SAMPLE_RSS).unwrap();

        assert_eq!(items.len(), 2);

        let first = &items[0];
        assert_eq!(
            first.title,
            "[SubsPlease] One Piece - 1060 (1080p) [37A98D45].mkv"
        );
        assert_eq!(
            first.torrent_link,
            "https://nyaa.si/download/2059096.torrent"
        );
        assert_eq!(first.view_url, "https://nyaa.si/view/2059096");
        assert_eq!(first.seeders, 18);
        assert_eq!(first.leechers, 8);
        assert_eq!(
            first.info_hash,
            "e30690d4a8d1f5e45f5ded430bdaedc710da0245"
        );
        assert_eq!(first.category_id, "1_2");
        assert_eq!(first.size, "1.2 GiB");
    }

    #[test]
    fn test_parse_episode_info_subsplease() {
        let title = "[SubsPlease] One Piece - 1060 (1080p) [37A98D45].mkv";
        let result = parse_episode_info(title);

        assert!(result.is_some());
        let (show, episode, quality) = result.unwrap();
        assert_eq!(show, "One Piece");
        assert_eq!(episode, 1060);
        assert_eq!(quality, "1080p");
    }

    #[test]
    fn test_parse_episode_info_erai_raws() {
        let title = "[Erai-raws] Goblin Slayer II - 03 [1080p][Multiple Subtitle] [ENG][POR-BR]";
        let result = parse_episode_info(title);

        assert!(result.is_some());
        let (show, episode, quality) = result.unwrap();
        assert_eq!(show, "Goblin Slayer II");
        assert_eq!(episode, 3);
        assert_eq!(quality, "1080p");
    }

    #[test]
    fn test_parse_episode_info_720p() {
        let title = "[SubsPlease] Frieren - 12 (720p) [ABC123].mkv";
        let result = parse_episode_info(title);

        assert!(result.is_some());
        let (show, episode, quality) = result.unwrap();
        assert_eq!(show, "Frieren");
        assert_eq!(episode, 12);
        assert_eq!(quality, "720p");
    }

    #[test]
    fn test_parse_episode_info_480p() {
        let title = "[SubsPlease] Some Anime - 05 (480p) [hash].mkv";
        let result = parse_episode_info(title);

        assert!(result.is_some());
        let (_, _, quality) = result.unwrap();
        assert_eq!(quality, "480p");
    }

    #[test]
    fn test_parse_episode_info_2160p() {
        let title = "[SubsPlease] 4K Anime - 01 (2160p) [hash].mkv";
        let result = parse_episode_info(title);

        assert!(result.is_some());
        let (_, _, quality) = result.unwrap();
        assert_eq!(quality, "2160p");
    }

    #[test]
    fn test_parse_episode_info_invalid() {
        let title = "Some random text without proper format";
        let result = parse_episode_info(title);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_episode_info_full_ordinal_season() {
        // Test "3rd Season" format (like Oshi no Ko)
        let title = "[Erai-raws] Oshi no Ko 3rd Season - 01 [1080p CR WEBRip HEVC AAC][MultiSub][E5D615AA]";
        let result = parse_episode_info_full(title);

        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.show_title, "Oshi no Ko");
        assert_eq!(info.season, Some(3));
        assert_eq!(info.episode, 1);
        assert_eq!(info.quality, "1080p");
    }

    #[test]
    fn test_parse_episode_info_full_ordinal_2nd_season() {
        // Test "2nd Season" format
        let title = "[SubsPlease] Blue Lock 2nd Season - 05 (1080p) [HASH].mkv";
        let result = parse_episode_info_full(title);

        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.show_title, "Blue Lock");
        assert_eq!(info.season, Some(2));
        assert_eq!(info.episode, 5);
        assert_eq!(info.quality, "1080p");
    }

    #[test]
    fn test_parse_episode_info_full_s_format() {
        // Test "S2" format
        let title = "[SubsPlease] Show Name S2 - 02 (1080p) [HASH].mkv";
        let result = parse_episode_info_full(title);

        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.show_title, "Show Name");
        assert_eq!(info.season, Some(2));
        assert_eq!(info.episode, 2);
        assert_eq!(info.quality, "1080p");
    }

    #[test]
    fn test_parse_episode_info_full_no_season() {
        // Test no season format (like long-running shows)
        let title = "[SubsPlease] One Piece - 1080 (1080p) [HASH].mkv";
        let result = parse_episode_info_full(title);

        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.show_title, "One Piece");
        assert_eq!(info.season, None);
        assert_eq!(info.episode, 1080);
        assert_eq!(info.quality, "1080p");
    }

    #[test]
    fn test_construct_magnet_url() {
        let info_hash = "e30690d4a8d1f5e45f5ded430bdaedc710da0245";
        let title = "One Piece - 1060";

        let magnet = construct_magnet_url(info_hash, title);

        assert!(magnet.starts_with("magnet:?xt=urn:btih:"));
        assert!(magnet.contains(info_hash));
        assert!(magnet.contains("One%20Piece"));
    }

    #[test]
    fn test_filter_by_quality() {
        let items = parse_rss_xml(SAMPLE_RSS).unwrap();

        let hd_items = filter_by_quality(&items, "1080p");
        assert_eq!(hd_items.len(), 1);
        assert!(hd_items[0].title.contains("1080p"));

        let sd_items = filter_by_quality(&items, "720p");
        assert_eq!(sd_items.len(), 1);
        assert!(sd_items[0].title.contains("720p"));
    }

    #[test]
    fn test_rss_item_to_parsed_episode() {
        let items = parse_rss_xml(SAMPLE_RSS).unwrap();
        let episode = rss_item_to_parsed_episode(&items[0]);

        assert!(episode.is_some());
        let ep = episode.unwrap();
        assert_eq!(ep.show_title, "One Piece");
        assert_eq!(ep.episode, 1060);
        assert_eq!(ep.quality, "1080p");
        assert!(!ep.magnet_url.is_empty());
        assert!(ep.magnet_url.starts_with("magnet:?xt=urn:btih:"));
    }

    #[ignore]
    #[tokio::test]
    async fn test_fetch_rss_feed_live() {
        // Integration test - requires network access
        let items = fetch_rss_feed("subsplease", "One Piece").await;
        assert!(items.is_ok());
        let items = items.unwrap();
        println!("Fetched {} items", items.len());
        for item in items.iter().take(3) {
            println!("  - {}", item.title);
        }
    }

    #[ignore]
    #[tokio::test]
    async fn test_fetch_episodes_live() {
        // Integration test - requires network access
        let episodes = fetch_episodes("subsplease", "One Piece", Some("1080p")).await;
        assert!(episodes.is_ok());
        let episodes = episodes.unwrap();
        println!("Fetched {} episodes", episodes.len());
        for ep in episodes.iter().take(3) {
            println!(
                "  - {} Episode {} ({})",
                ep.show_title, ep.episode, ep.quality
            );
        }
    }

    // SubsPlease RSS tests

    const SAMPLE_SUBSPLEASE_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>SubsPlease RSS</title>
    <item>
      <title>[SubsPlease] One Piece - 1100 (1080p) [ABC123].mkv</title>
      <link>https://nyaa.si/view/1234567</link>
      <guid>https://nyaa.si/view/1234567</guid>
      <pubDate>Thu, 16 Jan 2026 12:00:00 +0000</pubDate>
    </item>
    <item>
      <title>[SubsPlease] Frieren - 28 (1080p) [DEF456].mkv</title>
      <link>https://nyaa.si/view/1234568</link>
      <guid>https://nyaa.si/view/1234568</guid>
      <pubDate>Thu, 16 Jan 2026 11:00:00 +0000</pubDate>
    </item>
    <item>
      <title>[SubsPlease] Blue Lock - 24 (1080p) [GHI789].mkv</title>
      <link>https://nyaa.si/view/1234569</link>
      <guid>https://nyaa.si/view/1234569</guid>
      <pubDate>Thu, 16 Jan 2026 10:00:00 +0000</pubDate>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn test_parse_subsplease_rss_xml() {
        let items = parse_subsplease_rss_xml(SAMPLE_SUBSPLEASE_RSS).unwrap();

        assert_eq!(items.len(), 3);

        let first = &items[0];
        assert_eq!(
            first.title,
            "[SubsPlease] One Piece - 1100 (1080p) [ABC123].mkv"
        );
        assert_eq!(
            first.torrent_link,
            "https://nyaa.si/download/1234567.torrent"
        );
        assert_eq!(first.view_url, "https://nyaa.si/view/1234567");
    }

    #[test]
    fn test_rss_source_from_string() {
        assert_eq!(
            RssSource::from_source_string("subsplease_direct"),
            RssSource::SubsPleaseDirect
        );
        assert_eq!(
            RssSource::from_source_string("SUBSPLEASE_DIRECT"),
            RssSource::SubsPleaseDirect
        );
        assert_eq!(
            RssSource::from_source_string("subsplease"),
            RssSource::Nyaa
        );
        assert_eq!(RssSource::from_source_string("erai-raws"), RssSource::Nyaa);
        assert_eq!(RssSource::from_source_string(""), RssSource::Nyaa);
    }

    #[ignore]
    #[tokio::test]
    async fn test_fetch_subsplease_rss_live() {
        // Integration test - requires network access
        let items = fetch_subsplease_rss("1080").await;
        assert!(items.is_ok());
        let items = items.unwrap();
        println!("Fetched {} items from SubsPlease", items.len());
        for item in items.iter().take(5) {
            println!("  - {}", item.title);
        }
    }

    #[ignore]
    #[tokio::test]
    async fn test_fetch_rss_by_source_subsplease_live() {
        // Integration test - requires network access
        let items =
            fetch_rss_by_source(RssSource::SubsPleaseDirect, "", "One Piece", "1080p").await;
        assert!(items.is_ok());
        let items = items.unwrap();
        println!("Fetched {} One Piece items from SubsPlease", items.len());
        for item in &items {
            println!("  - {}", item.title);
        }
    }

    #[test]
    fn test_detect_fansub_source_subsplease() {
        assert_eq!(
            detect_fansub_source("[SubsPlease] One Piece - 1060 (1080p) [37A98D45].mkv"),
            "subsplease"
        );
    }

    #[test]
    fn test_detect_fansub_source_erai_raws() {
        assert_eq!(
            detect_fansub_source("[Erai-raws] Goblin Slayer II - 03 [1080p][Multiple Subtitle]"),
            "Erai-raws"
        );
    }

    #[test]
    fn test_detect_fansub_source_judas() {
        assert_eq!(
            detect_fansub_source("[Judas] Attack on Titan - The Final Season - 01.mkv"),
            "judas"
        );
    }

    #[test]
    fn test_detect_fansub_source_unknown() {
        // Unknown groups should preserve their original casing
        assert_eq!(
            detect_fansub_source("[SomeNewGroup] Anime - 01 (1080p).mkv"),
            "SomeNewGroup"
        );
    }

    #[test]
    fn test_detect_fansub_source_no_brackets() {
        // No brackets should default to subsplease
        assert_eq!(
            detect_fansub_source("One Piece - 1060 (1080p).mkv"),
            "subsplease"
        );
    }
}
