use std::collections::VecDeque;
use std::process::{Child, Command};

use anyhow::{anyhow, Ok, Result};
use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Prompt user input
    let (sp_title, season_number, batch) = get_user_input();
    // This needs a running web driver like chromedriver or geckodriver
    let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch).await?;
    let _result = upload_to_transmission_rpc(magnet_links, &sp_title, season_number).await?;

    Ok(())
}

fn get_user_input() -> (String, u8, bool) {
    let mut sp_title = String::new();
    println!("Enter the subsplease title: ");
    std::io::stdin()
        .read_line(&mut sp_title)
        .expect("Could not read arg");
    sp_title = sp_title.replace("–", "-").trim().to_string();

    let mut season_str = String::new();
    println!("Enter the season: ");
    std::io::stdin()
        .read_line(&mut season_str)
        .expect("Could not read arg");
    let season_number = season_str.trim().parse::<u8>().unwrap();

    let mut batch_str = String::new();
    println!("Enter true or false for batch download: ");
    std::io::stdin()
        .read_line(&mut batch_str)
        .expect("Could not read arg");
    let batch = batch_str.trim().parse::<bool>().unwrap();

    (sp_title, season_number, batch)
}

async fn get_magnet_links_from_subsplease(
    sp_title: &str,
    batch_if_available: bool,
) -> Result<Vec<String>> {
    let driver_process = DriverProcess::new("geckodriver", 4444);
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

async fn upload_to_transmission_rpc(
    links: Vec<String>,
    show_name: &str,
    season_number: u8,
) -> Result<()> {
    let url = "http://192.168.86.71:9091/transmission/rpc";

    let client = reqwest::Client::new();
    let resp = client.get(url).send().await?;
    let session_id = resp
        .headers()
        .get("X-Transmission-Session-Id")
        .ok_or_else(|| anyhow!("Missing X-Transmission-Session-Id header"))?
        .to_str()
        .map_err(|_| anyhow!("Invalid X-Transmission-Session-Id header"))?;

    println!("Recieved session_id from transmission");

    let destination_folder = format!("/data/Anime/{}/Season {}/", show_name, season_number);

    let count = links.len();
    for magnet_link in links {
        let body = format!(
            r#"{{
            "method": "torrent-add",
            "arguments": {{
                "filename": "{}",
                "download-dir": "{}"
            }}"#,
            magnet_link, destination_folder
        );

        let post_resp = client
            .post(url)
            .header("X-Transmission-Session-Id", session_id)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        println!(
            "Response for {}: {:?}",
            magnet_link,
            post_resp.text().await?
        );
    }

    println!("Sent links to transmission: {:?}", count);

    Ok(())
}

// RAII (Resource Acquisition Is Initialized)
// When this struct is dropped, the child process is terminated
struct DriverProcess {
    child: Option<Child>,
    port: u16,
}

impl DriverProcess {
    fn new(command: &str, desired_port: u16) -> Self {
        // Specify port for geckodriver
        let child = Command::new(command)
            .arg("-p")
            .arg(desired_port.to_string())
            .spawn()
            .expect("Failed to start driver");

        DriverProcess {
            child: Some(child),
            port: desired_port,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for DriverProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill(); // Kill child process
            let _ = child.wait(); // Wait for the process to terminate
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_scrape_subs_and_upload_batch_true() {
        let sp_title = "Sousou no Frieren".to_string();
        let season_number = 1;
        let batch = true;
        let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch).await;
        assert!(magnet_links.is_ok());
        let result =
            upload_to_transmission_rpc(magnet_links.unwrap(), &sp_title, season_number).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_scrape_subs_and_upload_batch_false() {
        let sp_title = "Sousou no Frieren".to_string();
        let season_number = 1;
        let batch = false;
        let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch).await;
        assert!(magnet_links.is_ok());
        let result =
            upload_to_transmission_rpc(magnet_links.unwrap(), &sp_title, season_number).await;
        assert!(result.is_ok());
    }
}
