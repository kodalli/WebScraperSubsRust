use anyhow::{self, Ok};
use reqwest;
use scraper::{Html, Selector};

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

pub async fn parse_nyaa(request_text: String) -> anyhow::Result<Vec<Torrent>> {
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
            println!("Title: {:?}", title);
            println!("View: {:?}", view_link);
            torrents.push(Torrent {
                title,
                view: view_link,
                torrent: torrent_link,
                magnet: magnet_link,
            });
        }
    }

    Ok(torrents)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_nyaa() {
        let keyword = "One Piece";
        let user = Some("subsplease");
        let request_text =
            get_torrents_from_nyaa(keyword, user, None, None, None, None, None, None).await;
        assert!(request_text.is_ok());
        let parsed = parse_nyaa(request_text.unwrap()).await;
        assert!(parsed.is_ok());
    }
}
