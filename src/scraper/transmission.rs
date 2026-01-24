use anyhow::{anyhow, Ok, Result};
use serde::Deserialize;

/// Response from Transmission RPC
#[derive(Debug, Deserialize)]
struct TransmissionResponse {
    result: String,
    #[serde(default)]
    arguments: Option<TorrentListArgs>,
}

#[derive(Debug, Deserialize)]
struct TorrentListArgs {
    #[serde(default)]
    torrents: Vec<TorrentInfo>,
}

#[derive(Debug, Deserialize)]
struct TorrentInfo {
    id: i64,
    name: String,
    #[serde(rename = "hashString", default)]
    hash_string: String,
}

/// Get the Transmission session ID for RPC calls
async fn get_session_id() -> Result<(&'static reqwest::Client, String, String)> {
    let host = std::env::var("TRANSMISSION_HOST").unwrap_or_else(|_| "192.168.86.71".to_string());
    let port = std::env::var("TRANSMISSION_PORT").unwrap_or_else(|_| "9091".to_string());
    let url = format!("http://{}:{}/transmission/rpc", host, port);

    let client = super::http_client();
    let resp = client.get(&url).send().await?;
    let session_id = resp
        .headers()
        .get("X-Transmission-Session-Id")
        .ok_or_else(|| anyhow!("Missing X-Transmission-Session-Id header"))?
        .to_str()
        .map_err(|_| anyhow!("Invalid X-Transmission-Session-Id header"))?
        .to_string();

    Ok((client, url, session_id))
}

/// Get all torrent hashes currently in Transmission
///
/// Returns a set of lowercase hash strings for quick lookup
pub async fn get_existing_torrent_hashes() -> Result<std::collections::HashSet<String>> {
    let (client, url, session_id) = get_session_id().await?;

    let body = r#"{"method": "torrent-get", "arguments": {"fields": ["id", "name", "hashString"]}}"#;

    let resp = client
        .post(&url)
        .header("X-Transmission-Session-Id", &session_id)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await?;

    let text = resp.text().await?;
    let response: TransmissionResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("Failed to parse torrent list: {} - Response: {}", e, text))?;

    let torrents = response
        .arguments
        .map(|a| a.torrents)
        .unwrap_or_default();

    let hashes: std::collections::HashSet<String> = torrents
        .into_iter()
        .map(|t| t.hash_string.to_lowercase())
        .collect();

    Ok(hashes)
}

/// Remove all torrents from Transmission, optionally deleting local data
///
/// # Arguments
/// * `delete_local_data` - If true, also deletes the downloaded files
///
/// # Returns
/// The number of torrents removed
pub async fn clear_all_torrents(delete_local_data: bool) -> Result<usize> {
    let (client, url, session_id) = get_session_id().await?;

    // First, get list of all torrents
    let list_body = r#"{"method": "torrent-get", "arguments": {"fields": ["id", "name"]}}"#;

    let list_resp = client
        .post(&url)
        .header("X-Transmission-Session-Id", &session_id)
        .header("Content-Type", "application/json")
        .body(list_body)
        .send()
        .await?;

    let list_text = list_resp.text().await?;
    let list_response: TransmissionResponse = serde_json::from_str(&list_text)
        .map_err(|e| anyhow!("Failed to parse torrent list: {} - Response: {}", e, list_text))?;

    let torrents = list_response
        .arguments
        .map(|a| a.torrents)
        .unwrap_or_default();

    if torrents.is_empty() {
        println!("No torrents to remove");
        return Ok(0);
    }

    let count = torrents.len();
    let torrent_ids: Vec<i64> = torrents.iter().map(|t| t.id).collect();

    println!("Removing {} torrent(s):", count);
    for t in &torrents {
        println!("  - {}", t.name);
    }

    // Remove all torrents
    let ids_json = serde_json::to_string(&torrent_ids)?;
    let remove_body = format!(
        r#"{{"method": "torrent-remove", "arguments": {{"ids": {}, "delete-local-data": {}}}}}"#,
        ids_json, delete_local_data
    );

    let remove_resp = client
        .post(&url)
        .header("X-Transmission-Session-Id", &session_id)
        .header("Content-Type", "application/json")
        .body(remove_body)
        .send()
        .await?;

    let remove_text = remove_resp.text().await?;
    let remove_response: TransmissionResponse = serde_json::from_str(&remove_text)
        .map_err(|e| anyhow!("Failed to parse remove response: {} - Response: {}", e, remove_text))?;

    if remove_response.result == "success" {
        println!("Successfully removed {} torrent(s)", count);
        Ok(count)
    } else {
        Err(anyhow!("Failed to remove torrents: {}", remove_response.result))
    }
}

pub async fn upload_to_transmission_rpc(
    links: Vec<String>,
    show_name: &str,
    season_number: Option<u8>,
) -> Result<()> {
    let (client, url, session_id) = get_session_id().await?;

    println!("Received session_id from transmission");


    let destination_folder = match season_number {
        Some(season) => format!("/data/Anime/{}/Season {}/", show_name, season),
        None => format!("/data/Anime/{}/", show_name),
    };

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
            .post(&url)
            .header("X-Transmission-Session-Id", &session_id)
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
