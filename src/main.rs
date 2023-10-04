use std::{
    panic,
    process::{Child, Command, Stdio}, io::Read,
};
use regex::Regex;

use thirtyfour::prelude::*;
use tokio::time::{sleep, Duration};

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
    let driver_process = DriverProcess::new("geckodriver");
    let port = driver_process.port();

    println!("Starting WebDriver");

    // Webscraper
    let caps = DesiredCapabilities::firefox();
    let driver = WebDriver::new(&format!("http://localhost:{}", port), caps)
        .await
        .expect("Failed to create WebDriver");
    let transmission_url = "http://192.168.86.71:9091/transmission/web/";
    driver
        .goto(transmission_url)
        .await
        .expect("Failed to navigate to URL");
    println!("Reached Transmission!");

    // close browser
    driver.quit().await.expect("Failed to quit WebDriver");

    Ok(())
}

// RAII (Resource Acquisition Is Initialized)
// When this struct is dropped, the child process is terminated

struct DriverProcess {
    child: Option<Child>,
    port: u16,
}

impl DriverProcess {
    fn new(command: &str) -> Self {
        // geckodriver chooses a random port :(
        let mut child = Command::new(command)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start driver");

        // delay parse
        let delay = sleep(Duration::from_secs(2));
        futures_executor::block_on(delay);

        let mut output_string = String::new();

        // read output
        if let Some(ref mut stdout) = child.stdout {
            let mut buffer = [0; 512];
            let bytes_read = stdout.read(&mut buffer).unwrap_or(0);
            output_string.push_str(std::str::from_utf8(&buffer[0..bytes_read]).unwrap_or_default());
        }

        // parse port with regex
        let re = Regex::new(r"Listening on port (\d+)").unwrap();
        let cap = re.captures(&output_string).expect("Failed to find port in output");
        let port: u16 = cap[1].parse().expect("Failed to parse port");

        DriverProcess { child: Some(child), port }
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

