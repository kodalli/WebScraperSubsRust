use crate::scraper::raii_process_driver::DriverProcess;
use anyhow::{Ok, Result};
use std::{collections::VecDeque, sync::Arc};
use thirtyfour::{By, DesiredCapabilities, WebDriver};

#[derive(Debug)]
pub struct Show {
    title: Arc<str>,
    magnet_links: Vec<Arc<str>>,
    episode_numbers: Vec<Arc<str>>,
    batch_link: Option<Arc<str>>,
    poster_link: Option<Arc<str>>,
}

pub async fn get_show_data_from_subsplease(sp_title: &str, geckoriver_port: u16) -> Result<Show> {
    let driver_process = DriverProcess::new("geckodriver", geckoriver_port);
    let port = driver_process.port();

    println!("Starting WebDriver");

    // WebDriver
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless()
        .expect("Failed to set browser to headless mode");
    let driver = WebDriver::new(&format!("http://localhost:{}", port), caps)
        .await
        .expect("Failed to create WebDriver");

    let subsplease_url = "https://subsplease.org/shows/";

    driver
        .goto(subsplease_url)
        .await
        .expect("Failed to navigate to URL");

    println!("Reached SubsPlease");

    let anime_xp = &format!("//a[text()=\"{}\"]", sp_title);
    let anime_elem = driver
        .find(By::XPath(anime_xp))
        .await
        .expect("Failed to find anime link");
    let anime_link = anime_elem
        .attr("href")
        .await
        .expect("Failed to find href attribute")
        .expect("Failed to find anime link");

    driver
        .goto(anime_link)
        .await
        .expect("Failed to navigate to Anime link");

    println!("Reached Anime Show Page");

    let batch_xp = "//h2[contains(text(), 'Batch')]";
    let batch_elem = driver.find(By::XPath(batch_xp)).await;

    // Get magnet links
    let magnet_xp = "//a[contains(@href,'1080p')]/span[text()='Magnet']/..";
    let magnet_elems = driver
        .find_all(By::XPath(magnet_xp))
        .await
        .expect("Failed to get magnet links");
    let mut magnet_links: VecDeque<String> = VecDeque::new();
    for mag_ele in magnet_elems {
        let result = mag_ele.attr("href").await?;
        if result.is_some() {
            magnet_links.push_back(result.unwrap());
        }
    }

    println!("Retrieved magnet links: {:?}", magnet_links.len());

    // Get episodes
    let episodes_xp = "//label[contains(@class, 'episode-title')]";
    let episodes_elems = driver
        .find_all(By::XPath(episodes_xp))
        .await
        .expect("Failed to get episode numbers");
    let mut episode_numbers: VecDeque<String> = VecDeque::new();
    for ep_ele in episodes_elems {
        let result = ep_ele.text().await?;
        episode_numbers.push_back(result);
    }

    println!("Retrieved episode numbers: {:?}", episode_numbers.len());

    // Get poster image
    let poster_xp = "//img[contains(@class, 'img-responsive')][contains(@class, 'img-center')]";
    let poster_elem = driver
        .find(By::XPath(poster_xp))
        .await
        .expect("Could not find poster image");
    let mut poster_link: Option<Arc<str>> = None;
    if let Some(link) = poster_elem.attr("src").await? {
        poster_link = Some(Arc::from(link.as_str()));
    }

    println!("Retrieved poster link");

    driver.quit().await.expect("Failed to quit WebDriver");

    // Filter batch link
    let mut batch_link: Option<Arc<str>> = None;
    if batch_elem.is_ok() {
        if let Some(link) = magnet_links.pop_front() {
            batch_link = Some(Arc::from(link.as_str()));
            let _ = episode_numbers.pop_front();
        }
    }

    println!("Final magnet links: {:?}", magnet_links.len());
    println!("Final episode numbers: {:?}", episode_numbers.len());

    let magnet_links = magnet_links.iter().map(|s| Arc::from(s.as_str())).collect();
    let episode_numbers = episode_numbers.iter().map(|s| Arc::from(s.as_str())).collect();

    return Ok(Show {
        title: Arc::from(sp_title),
        magnet_links,
        episode_numbers,
        batch_link,
        poster_link,
    });
}


pub async fn get_magnet_links_from_subsplease(
    sp_title: &str,
    batch_if_available: bool,
    geckoriver_port: u16,
) -> Result<Vec<String>> {
    let driver_process = DriverProcess::new("geckodriver", geckoriver_port);
    let port = driver_process.port();

    println!("Starting WebDriver");

    // WebDriver
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless()
        .expect("Failed to set browser to headless mode");
    let driver = WebDriver::new(&format!("http://localhost:{}", port), caps)
        .await
        .expect("Failed to create WebDriver");

    let subsplease_url = "https://subsplease.org/shows/";

    driver
        .goto(subsplease_url)
        .await
        .expect("Failed to navigate to URL");

    println!("Reached SubsPlease");

    let anime_xp = &format!("//a[text()=\"{}\"]", sp_title);
    let anime_elem = driver
        .find(By::XPath(anime_xp))
        .await
        .expect("Failed to find anime link");
    let anime_link = anime_elem
        .attr("href")
        .await
        .expect("Failed to find href attribute")
        .expect("Failed to find anime link");

    driver
        .goto(anime_link)
        .await
        .expect("Failed to navigate to Anime link");

    println!("Reached Anime Show Page");

    let batch_xp = "//h2[contains(text(), 'Batch')]";
    let batch_elem = driver.find(By::XPath(batch_xp)).await;

    let magnet_xp = "//a[contains(@href,'1080p')]/span[text()='Magnet']/..";
    let magnet_elems = driver
        .find_all(By::XPath(magnet_xp))
        .await
        .expect("Failed to get magnet links");

    let mut magnet_links: VecDeque<String> = VecDeque::new();

    for mag_ele in magnet_elems {
        let result = mag_ele.attr("href").await?;
        if result.is_some() {
            magnet_links.push_back(result.unwrap());
        }
    }

    println!("Retrieved magnet links: {:?}", magnet_links.len());

    if batch_elem.is_ok() {
        let batch_link = magnet_links.pop_front();
        if batch_if_available && batch_link.is_some() {
            magnet_links.clear();
            magnet_links.push_back(batch_link.unwrap());
        }
    }

    driver.quit().await.expect("Failed to quit WebDriver");

    println!("Final magnet links: {:?}", magnet_links.len());

    return Ok(magnet_links.into());
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_scrape_subs() {
        let sp_title = "Sousou no Frieren".to_string();
        let show = get_show_data_from_subsplease(&sp_title, 4445).await;
        assert!(show.is_ok());
        println!("show: {:?}", show.unwrap());
    }
}
