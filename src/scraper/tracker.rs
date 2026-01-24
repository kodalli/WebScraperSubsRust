//! Automated show tracker using RSS feeds and SQLite database
//!
//! This module handles the periodic polling of RSS feeds and automated
//! downloading of new episodes for tracked shows.

use anyhow::Result;
use chrono::{Duration, Local, TimeZone};
use std::time::SystemTime;
use tokio::time::sleep;

use crate::db::{self, models::Show};

use super::rss::{construct_magnet_url, fetch_rss_feed, parse_episode_info};
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

/// Process a single show: fetch RSS, filter by quality, and download new episodes
async fn process_show(show: &Show) -> Result<u32> {
    let mut downloaded_count = 0u32;

    // Fetch RSS feed for this show
    let rss_items = match fetch_rss_feed(&show.source, &show.alternate).await {
        Ok(items) => items,
        Err(e) => {
            eprintln!(
                "Failed to fetch RSS feed for '{}': {:?}",
                show.alternate, e
            );
            return Ok(0);
        }
    };

    if rss_items.is_empty() {
        println!("No RSS items found for '{}'", show.alternate);
        return Ok(0);
    }

    // Filter by quality preference and collect owned items
    let filtered_items: Vec<_> = rss_items
        .iter()
        .filter(|item| item.title.contains(&show.quality))
        .cloned()
        .collect();

    if filtered_items.is_empty() {
        println!(
            "No items matching quality '{}' for '{}'",
            show.quality, show.alternate
        );
        return Ok(0);
    }

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

    // Process each item
    for item in filtered_items {
        // Parse episode information from the title
        let (_, episode, _) = match parse_episode_info(&item.title) {
            Some(info) => info,
            None => {
                println!("Could not parse episode info from: {}", item.title);
                continue;
            }
        };

        // Skip episodes we've already downloaded (by episode number)
        if episode <= last_downloaded_episode {
            continue;
        }

        // Check if this specific torrent has already been downloaded (by hash)
        let info_hash = item.info_hash.clone();
        let already_downloaded = db::with_db(move |conn| {
            db::history::is_already_downloaded(conn, &info_hash)
        })
        .await?;

        if already_downloaded {
            println!("Already downloaded (by hash): {}", item.title);
            continue;
        }

        // Construct magnet URL for download
        let magnet_url = construct_magnet_url(&item.info_hash, &item.title);

        // Upload to Transmission
        match upload_to_transmission_rpc(
            vec![magnet_url.clone()],
            &show_alternate,
            Some(show_season),
        )
        .await
        {
            Ok(_) => {
                println!("Downloaded: {}", item.title);

                // Clone values for the closure
                let info_hash = item.info_hash.clone();
                let torrent_link = item.torrent_link.clone();

                // Record the download in history
                if let Err(e) = db::with_db(move |conn| {
                    db::history::record_download(
                        conn,
                        show_id,
                        episode,
                        &info_hash,
                        &torrent_link,
                    )
                })
                .await
                {
                    eprintln!("Failed to record download in history: {:?}", e);
                }

                // Clone hash again for the second closure
                let info_hash = item.info_hash.clone();

                // Update show's last downloaded episode
                if let Err(e) = db::with_db(move |conn| {
                    db::shows::update_last_downloaded(conn, show_id, episode, &info_hash)
                })
                .await
                {
                    eprintln!("Failed to update last downloaded episode: {:?}", e);
                }

                downloaded_count += 1;
            }
            Err(e) => {
                eprintln!("Failed to upload '{}' to Transmission: {:?}", item.title, e);
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
        println!("No tracked shows found in database.");
        return Ok(());
    }

    println!("Processing {} tracked show(s)...", shows.len());

    let mut total_downloaded = 0u32;

    for show in &shows {
        println!("Checking: {} ({})", show.title, show.quality);

        match process_show(show).await {
            Ok(count) => {
                total_downloaded += count;
            }
            Err(e) => {
                eprintln!("Error processing show '{}': {:?}", show.title, e);
            }
        }
    }

    // Update last poll time
    if let Err(e) = db::with_db(|conn| db::config::update_last_poll_time(conn)).await {
        eprintln!("Failed to update last poll time: {:?}", e);
    }

    println!(
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
                eprintln!("Failed to calculate next run time: {:?}", e);
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

        let mode = if rss_enabled { "RSS interval" } else { "fallback (5AM/5PM)" };
        println!(
            "Next poll in {:?} ({} mode)",
            wait_duration, mode
        );

        sleep(wait_duration).await;

        println!("Starting download check at {:?}", Local::now());

        match download_shows().await {
            Ok(_) => println!("Download check completed successfully."),
            Err(e) => eprintln!("Download check failed: {:?}", e),
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
