pub mod transmission;
pub mod nyaasi;
pub mod anilist;
pub mod tracker;
pub mod rss;
pub mod season_parser;
pub mod filter_engine;
mod raii_process_driver;

use reqwest::Client;
use std::sync::OnceLock;

/// Global HTTP client for connection pooling across all modules.
/// Using a shared client reduces memory usage by reusing connections.
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Returns a reference to the shared HTTP client.
/// The client is lazily initialized on first use with optimized settings
/// for low memory usage (limited idle connections per host).
pub fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .pool_max_idle_per_host(2) // Limit idle connections for memory savings
            .build()
            .expect("Failed to create HTTP client")
    })
}

