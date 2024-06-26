use anyhow::{self, Ok};
use reqwest;
use scraper::{Html, Selector};
use serde::Deserialize;

#[derive(Debug)]
pub struct Torrent {
    title: Option<String>,
    view: Option<String>,
    torrent: Option<String>,
    magnet: Option<String>,
}

pub async fn get_torrents_from_nyaa(
    keyword: &str,
    user: Option<&str>,
    filters: Option<u8>,
    category: Option<u8>,
    subcategory: Option<u8>,
    page: Option<u32>,
    sorting: Option<&str>,
    order: Option<&str>,
) -> anyhow::Result<String> {
    let uri = "https://nyaa.si";
    let user_uri = user.map_or("".into(), |s| format!("user/{}", s));
    let category = category.unwrap_or(0);
    let subcategory = subcategory.unwrap_or(0);
    let filters = filters.unwrap_or(0);
    let page = page.unwrap_or(0);
    let sorting = sorting.unwrap_or("id");
    let order = order.unwrap_or("desc");

    let resp = reqwest::get(format!(
        "{}/{}?f={}&c={}_{}&q={}&p={}&s={}&o={}",
        uri, user_uri, filters, category, subcategory, keyword, page, sorting, order
    ))
    .await?;
    let text = resp.text().await?;

    Ok(text)
}

pub fn parse_nyaa(request_text: String) -> Vec<Torrent> {
    // div/table/tbody/tr
    // td/a href -> view/id, title -> 1080p, separate (title, episode)
    // td/a href, class -> comments ignore
    // td/a href -> torrent, magnet

    let fragment = Html::parse_document(&request_text);

    // Select all 'tr' elements with a title containing "1080p"
    let selector_tr = Selector::parse("tr").unwrap();
    let selector_title = Selector::parse("a[title*='1080p']").unwrap();

    // Selectors for the different href values you want
    let selector_view = Selector::parse("a[href^='/view/']:not([href*='#comments'])").unwrap();
    let selector_torrent = Selector::parse("a[href*='.torrent']").unwrap();
    let selector_magnet = Selector::parse("a[href^='magnet']").unwrap();

    let mut torrents = Vec::new();
    for tr in fragment.select(&selector_tr) {
        if tr.select(&selector_title).next().is_some() {
            let mut view_link: Option<String> = None;
            let mut torrent_link: Option<String> = None;
            let mut magnet_link: Option<String> = None;
            let mut title: Option<String> = None;
            if let Some(view) = tr.select(&selector_view).next() {
                view_link = view.value().attr("href").map(|s| s.to_string());
                title = view.value().attr("title").map(|s| s.to_string());
            }
            if let Some(torrent) = tr.select(&selector_torrent).next() {
                torrent_link = torrent.value().attr("href").map(|s| s.to_string());
            }
            if let Some(magnet) = tr.select(&selector_magnet).next() {
                magnet_link = magnet.value().attr("href").map(|s| s.to_string());
            }
            torrents.push(Torrent {
                title,
                view: view_link,
                torrent: torrent_link,
                magnet: magnet_link,
            });
        }
    }

    return torrents;
}

#[derive(Deserialize, Debug)]
pub struct Link {
    pub title: String,
    pub episode: String,
    pub magnet_link: Option<String>,
    pub torrent_link: Option<String>,
}

fn extract_show_info(title: &str) -> (&str, &str) {
    let re = regex::Regex::new(r"\[.*?\] (.*?) - (\d+)").unwrap();

    if let Some(captures) = re.captures(title) {
        let show_title = captures.get(1).map_or("N/A", |m| m.as_str());
        let episode_number = captures.get(2).map_or("N/A", |m| m.as_str());
        return (show_title, episode_number);
    }

    ("N/A", "N/A")
}

pub async fn fetch_sources(keyword: &str, user: &str) -> anyhow::Result<Vec<Link>> {
    let local_user = if user != "default" {Some(user)} else {None};
    let request_text =
        get_torrents_from_nyaa(keyword, local_user, None, None, None, None, None, None).await?;
    let parsed = parse_nyaa(request_text);

    let links = parsed
        .iter()
        .filter_map(|p| {
            p.title.as_ref().and_then(|t| {
                let (title, episode) = extract_show_info(&t);
                Some(Link {
                    title: title.to_string(),
                    episode: episode.to_string(),
                    torrent_link: p.torrent.clone(),
                    magnet_link: p.magnet.clone(),
                })
            })
        })
        .collect::<Vec<Link>>();

    Ok(links)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::pages::home::{read_tracked_shows, TableEntry};

    use super::*;

    #[ignore]
    #[tokio::test]
    async fn test_nyaa_subsplease() {
        let keyword = "One Piece";
        let user = Some("subsplease");
        let request_text =
            get_torrents_from_nyaa(keyword, user, None, None, None, None, None, None).await;
        assert!(request_text.is_ok());
        let parsed = parse_nyaa(request_text.unwrap());
        assert!(parsed.len() > 0);
    }

    #[ignore]
    #[tokio::test]
    async fn test_nyaa_erai_raws() {
        let keyword = "One Piece";
        let user = Some("Erai-raws");
        let request_text =
            get_torrents_from_nyaa(keyword, user, Some(2), None, None, None, None, None).await;
        assert!(request_text.is_ok());
        let parsed = parse_nyaa(request_text.unwrap());
        assert!(parsed.len() > 0);
    }

    #[ignore]
    #[tokio::test]
    async fn test_nyaa_subsplease_json() {
        let map = match read_tracked_shows().await {
            anyhow::Result::Ok(map) => map,
            Err(err) => {
                eprintln!("{:?}", err);
                HashMap::new()
            }
        };
        let shows: Vec<&TableEntry> = map.values().collect();
        let keyword = shows.get(0).unwrap().title.as_str();
        //let user = Some("Erai-raws");
        let user = Some("subsplease");
        let request_text =
            get_torrents_from_nyaa(keyword, user, None, None, None, None, None, None).await;
        assert!(request_text.is_ok());
        //println!("{}", request_text.as_ref().unwrap());
        let parsed = parse_nyaa(request_text.unwrap());
        let titles: Vec<&String> = parsed.iter().map(|f| f.title.as_ref().unwrap()).collect();
        println!("{:?}", titles);
        println!("show: {}", keyword);
        assert!(parsed.len() > 0);
    }

    #[test]
    fn test_parse_title_episode() {
        let title1 = "[SubsPlease] One Piece - 1060 (1080p) [37A98D45].mkv";
        let title2 = "[Erai-raws] Goblin Slayer II - 03 [1080p][Multiple Subtitle] [ENG][POR-BR][SPA-LA][RUS]";

        let res1 = extract_show_info(title1);
        let res2 = extract_show_info(title2);

        println!("{:?}", res1);
        println!("{:?}", res2);
    }
}
