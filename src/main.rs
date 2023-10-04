use std::{
    panic,
    process::{Child, Command},
};

use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> WebDriverResult<()> {
    // This needs a running web driver like chromedriver or geckodriver

    let result = panic::catch_unwind(transmission);

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

    Ok(())
}

async fn transmission() -> WebDriverResult<()> {
    let driver_process = DriverProcess::new("geckodriver", 4444);
    let port = driver_process.port();

    println!("Starting WebDriver");

    // Webscraper
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless().expect("Failed to set browser to headless mode");
    let driver = WebDriver::new(&format!("http://localhost:{}", port), caps)
        .await
        .expect("Failed to create WebDriver");

    // Transmission
    let transmission_url = "http://192.168.86.71:9091/transmission/web/";
    driver
        .goto(transmission_url)
        .await
        .expect("Failed to navigate to URL");

    println!("Reached Transmission!");

    let open_xp = "//*[@id=\"toolbar-open\"]";
    //let torrent_upload_url_xp = "//*[@id=\"torrent_upload_url\"]";
    //let save_location_xp = "//*[@id=\"add-dialog-folder-input\"]";
    //let upload_xp = "//[@id=\"upload_confirm_button\"]";

    let upload_elem = driver.find(By::XPath(open_xp)).await.expect("Failed to find open button");
    upload_elem.click().await.expect("Failed to click open button");

    println!("Clicked Open Button!");

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
