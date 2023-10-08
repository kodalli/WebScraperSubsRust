use crate::scraper::raii_process_driver::DriverProcess;
use anyhow::{Ok, Result};
use std::collections::VecDeque;
use thirtyfour::{By, DesiredCapabilities, WebDriver};

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
