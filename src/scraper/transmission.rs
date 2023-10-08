use anyhow::{anyhow, Ok, Result};

pub async fn upload_to_transmission_rpc(
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
