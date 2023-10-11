use askama_axum::IntoResponse;
use axum::response::Html;

use crate::scraper::anilist::AniShow;
use crate::scraper::anilist::get_anilist_data;
use crate::scraper::anilist::Season;


pub async fn seasonal_anime() -> impl IntoResponse {
    // fetch fortune
    let res = match get_anilist_data(Season::FALL, 2023).await {
        Ok(res) => res,
        Err(err) => {
            println!("Failed to fetch seasonal anime. Error: {}", err);
            Vec::new()
        },
    };
    let show: AniShow = res.get(0).unwrap().to_owned();
    let title = show.title.unwrap().romaji.unwrap();
    Html(format!("\"{}\"", title))
}
