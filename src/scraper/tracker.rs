use chrono::{Local, Duration, TimeZone};
use std::{time::SystemTime, collections::HashMap};
use tokio::time::sleep;

use crate::pages::home::{read_tracked_data, TrackerDataEntry};

use super::{nyaasi::fetch_sources, transmission::upload_to_transmission_rpc};

fn next_run_time() -> SystemTime {
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

    let local_datetime = Local.from_local_datetime(&target_time).single().expect("Failed to convert naive datetime to local datetime");
    local_datetime.into()
}

pub async fn run_tracker() {
    loop {
        let now = SystemTime::now();
        let next_time = next_run_time();

        let wait_duration = next_time.duration_since(now).expect("time went backwards");
        sleep(wait_duration).await;

        // Execute your data fetch task here

        println!("Downloading Shows! {:?}", now);

        let data: HashMap<u32, TrackerDataEntry> = match read_tracked_data().await {
            Ok(val) => val,
            Err(err) => {
                println!("{:?}", err);
                HashMap::new()
            },
        };

        for (_, v) in data {
            let links = fetch_sources(&v.alternate, &v.source).await.unwrap_or_default();
            let latest = links.iter().max_by_key(|link| {
                let episode_number = link
                    .episode
                    .parse::<u16>()
                    .unwrap_or(0);
                episode_number
            });
            match latest {
                Some(val) => {
                    let url: Option<&str> = match (val.magnet_link.as_ref(), val.torrent_link.as_ref()) {
                        (None, None) => None,
                        (None, Some(url)) => Some(url),
                        (Some(url), None) => Some(url),
                        (Some(url), Some(_)) => Some(url),
                    };
                    if let Some(url) = url {
                        match upload_to_transmission_rpc(vec![url.to_string()], &v.alternate, Some(v.season)).await {
                            Ok(_) => println!("Downloaded {:?}", &v.alternate),
                            Err(err) => eprintln!("{:?}", err)
                        }
                    }
                }
                None => { println!("No latest episode link available.") }
            };
        }
    }
}


