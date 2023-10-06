mod raii_process_driver;
mod subsplease;
mod transmission;

use anyhow::{Ok, Result};
use subsplease::get_magnet_links_from_subsplease;
use transmission::upload_to_transmission_rpc;

#[tokio::main]
async fn main() -> Result<()> {
    // Prompt user input
    let (sp_title, season_number, batch) = get_user_input();
    // This needs a running web driver like chromedriver or geckodriver
    let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch, 4444).await?;
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

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_scrape_subs_and_upload_batch_true() {
        let sp_title = "Sousou no Frieren".to_string();
        let season_number = 1;
        let batch = true;
        let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch, 4444).await;
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
        let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch, 4445).await;
        assert!(magnet_links.is_ok());
        let result =
            upload_to_transmission_rpc(magnet_links.unwrap(), &sp_title, season_number).await;
        assert!(result.is_ok());
    }
}
