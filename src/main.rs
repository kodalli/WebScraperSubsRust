use std::{
    panic,
    process::{Child, Command},
};

use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> WebDriverResult<()> {

    // Prompt user input
    let mut sp_title = String::new();
    println!("Enter the subsplease title: ");
    std::io::stdin().read_line(&mut sp_title).expect("Could not read arg");
    sp_title = sp_title.replace("–", "-").trim().to_string();

    let mut season_str = String::new();
    println!("Enter the season: ");
    std::io::stdin().read_line(&mut season_str).expect("Could not read arg");
    let season_number = season_str.trim().parse::<u8>().unwrap();

    let mut batch_str = String::new();
    println!("Enter true or false for batch download: ");
    std::io::stdin().read_line(&mut batch_str).expect("Could not read arg");
    let batch = batch_str.trim().parse::<bool>().unwrap();

    // This needs a running web driver like chromedriver or geckodriver

    let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch).await;

    match magnet_links {
        Ok(links) => {
            for link in links {
                let result = panic::catch_unwind(|| async {
                    upload_to_transmission(&link, &sp_title, season_number).await
                });

                match result {
                    Ok(future_result) => {
                        let web_driver_result = future_result.await;
                        if let Err(e) = web_driver_result {
                            eprintln!("Error in WebDriver logic: {:?}", e);
                        }
                    }
                    Err(panic_info) => {
                        eprintln!("Program panicked: {:?}", panic_info);
                    }
                }
            }
        },
        Err(err) => eprintln!("Failed to get magnet links {:?}", err),
    }

    Ok(())
}

async fn get_magnet_links_from_subsplease(sp_title: &str, batch_if_available: bool) -> WebDriverResult<Vec<String>> {
    let driver_process = DriverProcess::new("geckodriver", 4444);
    let port = driver_process.port();

    println!("Starting WebDriver");

    // WebDriver
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless().expect("Failed to set browser to headless mode");
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
    let anime_elem = driver.find(By::XPath(anime_xp)).await.expect("Failed to find anime link");
    let anime_link = anime_elem.attr("href").await.expect("Failed to find href attribute").expect("Failed to find anime link");

    driver
        .goto(anime_link)
        .await
        .expect("Failed to navigate to Anime link");

    println!("Reached Anime Show Page");

    let batch_xp = "//h2[contains(text(), 'Batch')]";
    let batch_elem = driver.find(By::XPath(batch_xp)).await;

    let magnet_xp = "//a[contains(@href,'1080p')]/span[text()='Magnet']/..";
    let magnet_elems = driver.find_all(By::XPath(magnet_xp)).await;

    let mut magnet_links: Vec<String> = Vec::new();
    match magnet_elems {
       Ok(mag_elems) => {
            if batch_if_available && batch_elem.is_ok() {
                let mag_link = mag_elems.get(0).unwrap().attr("href").await;
                if mag_link.is_ok() {
                    magnet_links.push(mag_link.unwrap().unwrap());
                }
            } else {
                for mag_elem in mag_elems {
                    let mag_link = mag_elem.attr("href").await;
                    if mag_link.is_ok() {
                        magnet_links.push(mag_link.unwrap().unwrap());
                    }
                }
            }
            println!("Retrieved Magent Links");
       },
       Err(err) => {
           eprintln!("Could not find magnet links: {:?}", err);
       }
    }
    return Ok(magnet_links)
}

async fn upload_to_transmission(link: &str, show_name: &str, season_number: u8) -> WebDriverResult<()> {
    let driver_process = DriverProcess::new("geckodriver", 4444);
    let port = driver_process.port();

    println!("Starting WebDriver");

    // WebDriver
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless().expect("Failed to set browser to headless mode");
    let driver = WebDriver::new(&format!("http://localhost:{}", port), caps)
        .await
        .expect("Failed to create WebDriver");

    let transmission_url = "http://192.168.86.71:9091/transmission/web/";
    driver
        .goto(transmission_url)
        .await
        .expect("Failed to navigate to URL");

    println!("Reached Transmission!");

    let open_xp = "//*[@id=\"toolbar-open\"]";
    let torrent_upload_url_xp = "//*[@id=\"torrent_upload_url\"]";
    let save_location_xp = "//*[@id=\"add-dialog-folder-input\"]";
    let upload_xp = "//*[@id=\"upload_confirm_button\"]";

    let upload_elem = driver.find(By::XPath(open_xp)).await.expect("Failed to find open button");
    upload_elem.click().await.expect("Failed to click open button");

    println!("Clicked Open Button!");

    let url_elem = driver.find(By::XPath(torrent_upload_url_xp)).await.expect("Failed to find url element");
    url_elem.clear().await.expect("Failed to clear url element");
    url_elem.send_keys(link).await.expect("Failed to update url element");

    let destination_elem = driver.find(By::XPath(save_location_xp)).await.expect("Failed to find folder element");
    destination_elem.clear().await.expect("Failed to clear folder element");
    destination_elem.send_keys(&format!("/data/Anime/{}/Season {}/", show_name, season_number)).await.expect("Failed to update folder element");

    let upload_button = driver.find(By::XPath(upload_xp)).await.expect("Failed to find upload button");
    upload_button.click().await.expect("Failed to click upload button");

    println!("Queued torrent for {}", show_name);

    // Close Browser
    driver.quit().await.expect("Failed to quit WebDriver");

    println!("Closed WebDriver");

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
        // geckodriver chooses a random port :(
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
