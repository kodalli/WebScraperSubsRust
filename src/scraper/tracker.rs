//! Automated show tracker using RSS feeds and SQLite database
//!
//! This module handles the periodic polling of RSS feeds and automated
//! downloading of new episodes for tracked shows.

use anyhow::Result;
use chrono::{Duration, Local, TimeZone};
use std::time::SystemTime;
use tokio::time::sleep;

use crate::db::{self, models::Show};

use super::filter_engine::FilterEngine;
use super::rss::{construct_magnet_url, fetch_rss_by_source, parse_episode_info, RssSource};
use super::transmission::upload_to_transmission_rpc;

/// Calculate the next run time based on RSS config
///
/// If RSS is enabled, calculates based on poll_times_per_day.
/// Falls back to 5AM/5PM schedule if RSS is disabled.
async fn calculate_next_run_time() -> Result<(SystemTime, bool)> {
    let config = db::with_db(|conn| db::config::get_rss_config(conn)).await?;

    if config.enabled && config.poll_times_per_day > 0 {
        // Calculate interval based on polls per day
        let hours_between_polls = 24.0 / config.poll_times_per_day as f64;
        let duration_hours = hours_between_polls as i64;
        let duration_minutes = ((hours_between_polls - duration_hours as f64) * 60.0) as i64;

        let now = Local::now();
        let next_time = now + Duration::hours(duration_hours) + Duration::minutes(duration_minutes);

        Ok((next_time.into(), true))
    } else {
        // Fallback to 5AM/5PM schedule
        Ok((next_run_time_fallback(), false))
    }
}

/// Fallback schedule: next 5AM or 5PM
fn next_run_time_fallback() -> SystemTime {
    let now = Local::now();
    let today = now.date_naive();
    let next_5_am = today.and_hms_opt(5, 0, 0).expect("Invalid time");
    let next_5_pm = today.and_hms_opt(17, 0, 0).expect("Invalid time"); // 17:00 is 5:00 PM

    // Determine which one is the next target
    let target_time = if now.naive_local() < next_5_am {
        next_5_am
    } else if now.naive_local() < next_5_pm {
        next_5_pm
    } else {
        // If the current time is after 5:00 PM, the next target is 5:00 AM of the next day
        next_5_am + Duration::days(1)
    };

    let local_datetime = Local
        .from_local_datetime(&target_time)
        .single()
        .expect("Failed to convert naive datetime to local datetime");
    local_datetime.into()
}

/// Process a single show: fetch RSS, apply filters, and download new episodes
async fn process_show(show: &Show) -> Result<u32> {
    let mut downloaded_count = 0u32;

    // Determine RSS source type from show.source field
    let rss_source = RssSource::from_source_string(&show.source);

    // Fetch RSS feed using appropriate source
    let rss_items = match fetch_rss_by_source(
        rss_source,
        &show.source,
        &show.alternate,
        &show.quality,
    )
    .await
    {
        Ok(items) => items,
        Err(e) => {
            tracing::error!(
                "Failed to fetch RSS feed for '{}' (source: {:?}): {:?}",
                show.alternate,
                rss_source,
                e
            );
            return Ok(0);
        }
    };

    if rss_items.is_empty() {
        tracing::debug!("No RSS items found for '{}'", show.alternate);
        return Ok(0);
    }

    tracing::debug!(
        "Fetched {} RSS items for '{}' (source: {:?})",
        rss_items.len(),
        show.alternate,
        rss_source
    );

    // Load global filters from database
    let global_filters = db::with_db(|conn| db::get_global_filters(conn)).await?;

    // Load show-specific filter overrides
    let show_id_for_filters = show.id;
    let show_filters =
        db::with_db(move |conn| db::get_show_filters(conn, show_id_for_filters)).await?;

    // Create filter engine and apply filters
    let engine = FilterEngine::new(global_filters, show_filters);
    let filtered_results = engine.apply(rss_items);

    if filtered_results.is_empty() {
        tracing::debug!(
            "No items passed filters for '{}' (quality: {})",
            show.alternate,
            show.quality
        );
        return Ok(0);
    }

    tracing::debug!(
        "{} items passed filters for '{}'",
        filtered_results.len(),
        show.alternate
    );

    // Clone show data for use in closures (needed for 'static lifetime)
    let show_id = show.id;
    // Use title as fallback if alternate is empty
    let show_alternate = if show.alternate.is_empty() {
        show.title.clone()
    } else {
        show.alternate.clone()
    };
    let show_season = show.season;
    let last_downloaded_episode = show.last_downloaded_episode;

    // Process items in score order (highest first)
    for result in filtered_results {
        let item = result.item;

        // Log matched rules for debugging
        if !result.matched_rules.is_empty() {
            tracing::debug!(
                "Item '{}' matched rules: {:?}",
                item.title,
                result.matched_rules
            );
        }

        // Parse episode information from the title
        let (_, episode, _) = match parse_episode_info(&item.title) {
            Some(info) => info,
            None => {
                tracing::debug!("Could not parse episode info from: {}", item.title);
                continue;
            }
        };

        // Skip episodes we've already downloaded (by episode number)
        if episode <= last_downloaded_episode {
            continue;
        }

        // Check if this specific torrent has already been downloaded (by hash)
        // For SubsPlease direct RSS, info_hash might be empty, so use torrent_link as fallback
        let check_hash = if item.info_hash.is_empty() {
            // Use a hash of the torrent link as a unique identifier
            format!("subsplease:{}", item.torrent_link)
        } else {
            item.info_hash.clone()
        };

        let check_hash_clone = check_hash.clone();
        let already_downloaded = db::with_db(move |conn| {
            db::history::is_already_downloaded(conn, &check_hash_clone)
        })
        .await?;

        if already_downloaded {
            tracing::debug!("Already downloaded (by hash): {}", item.title);
            continue;
        }

        // Determine download URL: prefer magnet, fallback to torrent file
        let download_url = if !item.info_hash.is_empty() {
            construct_magnet_url(&item.info_hash, &item.title)
        } else {
            // For SubsPlease direct RSS, use the torrent URL directly
            item.torrent_link.clone()
        };

        // Upload to Transmission
        match upload_to_transmission_rpc(
            vec![download_url.clone()],
            &show_alternate,
            Some(show_season),
        )
        .await
        {
            Ok(_) => {
                tracing::info!("Downloaded: {}", item.title);

                // Clone values for the closure
                let record_hash = check_hash.clone();
                let torrent_link = item.torrent_link.clone();

                // Record the download in history
                if let Err(e) = db::with_db(move |conn| {
                    db::history::record_download(
                        conn,
                        show_id,
                        episode,
                        &record_hash,
                        &torrent_link,
                    )
                })
                .await
                {
                    tracing::error!("Failed to record download in history: {:?}", e);
                }

                // Update show's last downloaded episode
                let update_hash = check_hash.clone();
                if let Err(e) = db::with_db(move |conn| {
                    db::shows::update_last_downloaded(conn, show_id, episode, &update_hash)
                })
                .await
                {
                    tracing::error!("Failed to update last downloaded episode: {:?}", e);
                }

                downloaded_count += 1;
            }
            Err(e) => {
                tracing::error!("Failed to upload '{}' to Transmission: {:?}", item.title, e);
            }
        }
    }

    Ok(downloaded_count)
}

/// Download shows for all tracked entries using RSS feeds
pub async fn download_shows() -> Result<()> {
    // Get all tracked shows from SQLite
    let shows = db::with_db(|conn| db::shows::get_tracked_shows(conn)).await?;

    if shows.is_empty() {
        tracing::info!("No tracked shows found in database.");
        return Ok(());
    }

    tracing::info!("Processing {} tracked show(s)...", shows.len());

    let mut total_downloaded = 0u32;

    for show in &shows {
        tracing::debug!(
            "Checking: {} ({}) [source: {}]",
            show.title,
            show.quality,
            show.source
        );

        match process_show(show).await {
            Ok(count) => {
                total_downloaded += count;
            }
            Err(e) => {
                tracing::error!("Error processing show '{}': {:?}", show.title, e);
            }
        }
    }

    // Update last poll time
    if let Err(e) = db::with_db(|conn| db::config::update_last_poll_time(conn)).await {
        tracing::error!("Failed to update last poll time: {:?}", e);
    }

    tracing::info!(
        "Poll complete. Downloaded {} new episode(s).",
        total_downloaded
    );

    Ok(())
}

/// Run the tracker loop
///
/// Continuously polls RSS feeds at the configured interval and downloads new episodes.
pub async fn run_tracker() {
    loop {
        let now = SystemTime::now();

        // Calculate next run time based on config
        let (next_time, rss_enabled) = match calculate_next_run_time().await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Failed to calculate next run time: {:?}", e);
                // Fallback to fixed schedule on error
                (next_run_time_fallback(), false)
            }
        };

        let wait_duration = match next_time.duration_since(now) {
            Ok(duration) => duration,
            Err(_) => {
                // If next_time is in the past, run immediately
                std::time::Duration::from_secs(1)
            }
        };

        let mode = if rss_enabled {
            "RSS interval"
        } else {
            "fallback (5AM/5PM)"
        };
        tracing::info!("Next poll in {:?} ({} mode)", wait_duration, mode);

        sleep(wait_duration).await;

        tracing::info!("Starting download check at {:?}", Local::now());

        match download_shows().await {
            Ok(_) => tracing::info!("Download check completed successfully."),
            Err(e) => tracing::error!("Download check failed: {:?}", e),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database initialization
    async fn test_tracker() {
        // Initialize database before running
        // db::init_connection().expect("Failed to init DB");
        // db::init_database(&conn).expect("Failed to init schema");

        match download_shows().await {
            Ok(()) => {
                println!("Download check completed successfully");
            }
            Err(err) => {
                println!("Error: {:?}", err);
                panic!("Test failed");
            }
        };
    }

    #[test]
    fn test_fallback_schedule() {
        let next = next_run_time_fallback();
        let now = SystemTime::now();

        // Next run time should be in the future
        assert!(next > now);
    }
}
