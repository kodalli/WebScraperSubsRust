use crate::{
    pages::{filters, HtmlTemplate},
    scraper::{
        anilist::{get_anilist_all_airing, get_anilist_data, AniShow, NextAiringEpisode, Season},
        nyaasi::{fetch_sources, Link},
        transmission::upload_to_transmission_rpc,
    },
};
use anyhow::Ok;
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
    Form,
};
use chrono::{Datelike, NaiveDateTime, Utc};
use core::fmt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};

#[derive(Deserialize)]
pub struct UserState {
    pub user: String,
    pub tracker: HashMap<u32, TableEntry>,
    pub season: Season,
    pub year: u16,
}

#[derive(Deserialize)]
pub struct SeasonalAnimeQuery {
    pub season: String,
    pub year: u16,
}

#[derive(Deserialize)]
pub struct AnimeIdQuery {
    pub id: u32,
}

#[derive(Deserialize)]
pub struct AnimeKeywordQuery {
    pub keyword: String,
    pub source: String,
}

#[derive(Deserialize)]
pub struct DownloadAnimeQuery {
    pub title: String,
    pub url: String,
    pub season: Option<u8>,
}

#[derive(Deserialize, Serialize)]
pub struct TrackerDataEntry {
    pub title: String,
    pub id: u32,
    pub alternate: String,
    pub season: u8,
    pub source: String,
}

impl UserState {
    pub fn new(user: String, tracker: HashMap<u32, TableEntry>) -> Self {
        let (season, year) = current_year_and_season();
        Self {
            user,
            tracker,
            season,
            year,
        }
    }
}

#[derive(Template)]
#[template(path = "pages/home.html")]
pub struct HomeTemplate {
    pub user: String,
    pub grid: GridTemplate,
    pub navbar: NavBarTemplate,
    pub table: TableTemplate,
    pub season: Season,
    pub year: u16,
}

#[derive(Template)]
#[template(path = "components/card.html")]
pub struct CardTemplate {
    pub show: AniShow,
    pub tracker: TrackedTemplate,
}

#[derive(Template)]
#[template(path = "components/grid.html")]
pub struct GridTemplate {
    pub cards: Vec<CardTemplate>,
}

#[derive(Template)]
#[template(path = "components/season_bar.html")]
pub struct NavBarTemplate {
    pub seasons: Vec<(Season, u16)>,
}

#[derive(Template)]
#[template(path = "components/table.html")]
pub struct TableTemplate {
    shows: Vec<TableEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableEntry {
    pub title: String,
    pub latest_episode: String,
    pub next_air_date: String,
    pub is_tracked: bool,
    pub id: u32,
}

#[derive(Template)]
#[template(path = "components/tracked.html")]
pub struct TrackedTemplate {
    entry: TableEntry,
}

#[derive(Template)]
#[template(path = "components/tracked.html")]
pub struct TrackedTableTemplate {
    entry: TableEntry,
    pub table: TableTemplate,
}

#[derive(Template)]
#[template(path = "components/source_table.html")]
pub struct SourceTableTemplate {
    pub keyword: String,
    pub links: Vec<Link>,
}

#[derive(Template)]
#[template(path = "components/configure.html")]
pub struct ConfigureTemplate {
    pub title: String,
    pub id: u32,
    pub alternate: String,
    pub season: u8,
    pub source: String,
}

async fn get_seasonal(season: Season, year: u16) -> anyhow::Result<Vec<AniShow>> {
    match get_anilist_data(season, year).await {
        std::result::Result::Ok(res) => Ok(res),
        std::result::Result::Err(err) => {
            println!("Failed to fetch seasonal anime. Error: {}", err);
            Ok(Vec::new())
        }
    }
}

async fn get_currently_airing() -> anyhow::Result<Vec<AniShow>> {
    match get_anilist_all_airing().await {
        std::result::Result::Ok(res) => Ok(res),
        std::result::Result::Err(err) => {
            println!("Failed to fetch seasonal anime. Error: {}", err);
            Ok(Vec::new())
        }
    }
}

fn current_year_and_season() -> (Season, u16) {
    let now = Utc::now();
    let year = now.year() as u16;
    let month = now.month();

    let season = match month {
        1..=3 => Season::WINTER,
        4..=6 => Season::SPRING,
        7..=9 => Season::SUMMER,
        10..=12 => Season::FALL,
        _ => {
            println!("Unexpected month value {:?}, using fall season", month);
            Season::FALL
        }
    };

    (season, year)
}

impl fmt::Display for Season {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Season::SPRING => "Spring",
                Season::FALL => "Fall",
                Season::WINTER => "Winter",
                Season::SUMMER => "Summer",
            }
        )
    }
}

fn get_seasons_around(season: Season, year: u16) -> Vec<(Season, u16)> {
    let mut seasons = match season {
        Season::WINTER => vec![
            (Season::SUMMER, year - 1),
            (Season::FALL, year - 1),
            (Season::WINTER, year),
            (Season::SPRING, year),
        ],
        Season::SPRING => vec![
            (Season::FALL, year - 1),
            (Season::WINTER, year),
            (Season::SPRING, year),
            (Season::SUMMER, year),
        ],
        Season::SUMMER => vec![
            (Season::WINTER, year),
            (Season::SPRING, year),
            (Season::SUMMER, year),
            (Season::FALL, year),
        ],
        Season::FALL => vec![
            (Season::SPRING, year),
            (Season::SUMMER, year),
            (Season::FALL, year),
            (Season::WINTER, year + 1),
        ],
    };
    seasons.push(current_year_and_season());

    seasons
}

fn get_next_airing_episode(next_airing_episode: &Option<NextAiringEpisode>) -> (String, String) {
    let nae = match next_airing_episode {
        Some(ep) => ep,
        None => return ("N/A".into(), "N/A".into()),
    };

    let episode = nae
        .episode
        .map_or_else(|| "N/A".to_string(), |e| format!("Episode {}", e));
    let air_date = nae.airing_at.map_or_else(
        || "N/A".to_string(),
        |d| match NaiveDateTime::from_timestamp_opt(d, 0) {
            Some(date) => format!("{}", date),
            None => "N/A".into(),
        },
    );

    (episode, air_date)
}

fn build_card_templates(shows: &[AniShow], lock: &UserState) -> Vec<CardTemplate> {
    shows
        .iter()
        .map(|show| {
            let title = show.title.as_ref().map_or("N/A".into(), |t| {
                t.romaji.as_deref().unwrap_or("N/A").into()
            });

            let tracked = lock.tracker.get(&show.id.unwrap()).is_some();
            let (latest_episode, next_air_date) =
                get_next_airing_episode(&show.next_airing_episode);

            CardTemplate {
                show: show.clone(),
                tracker: TrackedTemplate {
                    entry: TableEntry {
                        title,
                        latest_episode,
                        next_air_date,
                        is_tracked: tracked,
                        id: show.id.unwrap(),
                    },
                },
            }
        })
        .collect()
}

pub async fn view(State(state): State<Arc<Mutex<UserState>>>) -> impl IntoResponse {
    let lock = state.lock().await;
    let shows: Vec<AniShow> = get_seasonal(lock.season, lock.year)
        .await
        .unwrap_or_default();
    let card_templates: Vec<CardTemplate> = build_card_templates(&shows, &lock);
    let grid_template = GridTemplate {
        cards: card_templates,
    };
    let seasons = get_seasons_around(lock.season, lock.year);
    let template = HomeTemplate {
        user: lock.user.clone(),
        grid: grid_template,
        season: lock.season,
        year: lock.year,
        navbar: NavBarTemplate { seasons },
        table: TableTemplate {
            shows: lock.tracker.values().cloned().collect(),
        },
    };
    HtmlTemplate::new(template)
}

#[axum::debug_handler]
pub async fn navigate_seasonal_anime(
    State(state): State<Arc<Mutex<UserState>>>,
    Form(payload): Form<SeasonalAnimeQuery>,
) -> impl IntoResponse {
    let mut lock = state.lock().await;
    println!(
        "season {}, year {}",
        payload.season.to_uppercase(),
        payload.year
    );
    let json_str = &format!("\"{}\"", payload.season.to_uppercase());
    lock.season = match serde_json::from_str(json_str) {
        core::result::Result::Ok(val) => val,
        Err(err) => {
            println!("{:?}", err);
            Season::SPRING
        }
    };
    lock.year = payload.year;

    let cards: Vec<AniShow> = get_seasonal(lock.season, lock.year)
        .await
        .unwrap_or_default();

    let card_templates: Vec<CardTemplate> = build_card_templates(&cards, &lock);

    let grid = GridTemplate {
        cards: card_templates,
    };

    HtmlTemplate::new(grid)
}

#[axum::debug_handler]
pub async fn currently_airing_anime(
    State(state): State<Arc<Mutex<UserState>>>,
) -> impl IntoResponse {
    let lock = state.lock().await;
    let cards: Vec<AniShow> = get_currently_airing().await.unwrap_or_default();
    let card_templates: Vec<CardTemplate> = build_card_templates(&cards, &lock);
    let grid = GridTemplate {
        cards: card_templates,
    };
    HtmlTemplate::new(grid)
}

#[axum::debug_handler]
pub async fn update_user(
    State(state): State<Arc<Mutex<UserState>>>,
    Form(payload): Form<UserState>,
) -> impl IntoResponse {
    let mut lock = state.lock().await;
    lock.user = payload.user.to_string();

    Html(format!("{}", payload.user))
}

pub async fn read_tracked_shows() -> anyhow::Result<HashMap<u32, TableEntry>> {
    let mut file = File::open("tracked_shows.json").await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;
    match serde_json::from_slice(&buffer) {
        anyhow::Result::Ok(map) => Ok(map),
        Err(err) => {
            println!("Failed to read tracked shows from json: {:?}", err);
            Ok(HashMap::new())
        }
    }
}

async fn save_tracked_shows(tracker: &HashMap<u32, TableEntry>) -> anyhow::Result<()> {
    let json_data = serde_json::to_string(tracker).expect("Failed to serialize hashmap");
    let mut file = File::create("tracked_shows.json").await?;
    file.write_all(json_data.as_bytes()).await?;
    Ok(())
}

#[axum::debug_handler]
pub async fn set_tracker(
    State(state): State<Arc<Mutex<UserState>>>,
    Query(payload): Query<TableEntry>,
) -> impl IntoResponse {
    let mut lock = state.lock().await;
    let mut new_payload = payload.clone();
    new_payload.is_tracked = !new_payload.is_tracked;
    if new_payload.is_tracked {
        lock.tracker.insert(new_payload.id, new_payload.clone());
    } else {
        lock.tracker.remove(&new_payload.id);
    }
    let template = TrackedTemplate { entry: new_payload };

    // Save the updated tracker hashmap to a JSON file
    match save_tracked_shows(&lock.tracker).await {
        anyhow::Result::Ok(_) => {}
        Err(err) => eprintln!("Could not save hashmap to json: {:?}", err),
    }

    HtmlTemplate::new(template).with_header("HX-Trigger", "newTrackerStatus")
}

#[axum::debug_handler]
pub async fn show_table(State(state): State<Arc<Mutex<UserState>>>) -> impl IntoResponse {
    let lock = state.lock().await;

    let template = TableTemplate {
        shows: lock.tracker.values().cloned().collect(),
    };
    HtmlTemplate::new(template)
}

#[axum::debug_handler]
pub async fn get_source(
    State(state): State<Arc<Mutex<UserState>>>,
    Query(payload): Query<AnimeIdQuery>,
) -> impl IntoResponse {
    println!("Get Source!");
    let lock = state.lock().await;
    let show = lock.tracker.get(&payload.id);
    let title = &show.unwrap().title;
    let links = match fetch_sources(title, "subsplease").await {
        anyhow::Result::Ok(val) => val,
        Err(err) => {
            println!("Couldn't fetch source for {}, {:?}", title, err);
            Vec::new()
        }
    };
    let template = SourceTableTemplate {
        keyword: title.clone(),
        links,
    };
    HtmlTemplate::new(template)
}

#[axum::debug_handler]
pub async fn search_source(Form(payload): Form<AnimeKeywordQuery>) -> impl IntoResponse {
    println!("Search!");
    let links = match fetch_sources(&payload.keyword, &payload.source).await {
        anyhow::Result::Ok(val) => val,
        Err(err) => {
            println!("Couldn't fetch source for {}, {:?}", &payload.keyword, err);
            Vec::new()
        }
    };
    let template = SourceTableTemplate {
        keyword: payload.keyword.clone(),
        links,
    };
    HtmlTemplate::new(template)
}

#[axum::debug_handler]
pub async fn download_from_link(Query(payload): Query<DownloadAnimeQuery>) -> impl IntoResponse {
    let links = vec![payload.url];
    let show_name = &payload.title;
    let season_number = payload.season;
    match upload_to_transmission_rpc(links, show_name, season_number).await {
        anyhow::Result::Ok(_) => println!("Successful Download! {}", show_name),
        Err(err) => eprintln!("Failed to download {:?}", err),
    };
    Html("Done")
}

pub async fn read_tracked_data() -> anyhow::Result<HashMap<u32, TrackerDataEntry>> {
    let mut file = File::open("tracked_data.json").await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;
    match serde_json::from_slice(&buffer) {
        anyhow::Result::Ok(map) => Ok(map),
        Err(err) => {
            println!("Failed to read tracked data from json: {:?}", err);
            Ok(HashMap::new())
        }
    }
}

async fn save_tracked_data(tracker: &HashMap<u32, TrackerDataEntry>) -> anyhow::Result<()> {
    let json_data = serde_json::to_string(tracker).expect("Failed to serialize hashmap");
    let mut file = File::create("tracked_data.json").await?;
    file.write_all(json_data.as_bytes()).await?;
    Ok(())
}

#[axum::debug_handler]
pub async fn get_configuration(
    State(state): State<Arc<Mutex<UserState>>>,
    Query(payload): Query<AnimeIdQuery>,
) -> impl IntoResponse {
    let data = read_tracked_data().await.unwrap();
    let template = match data.get(&payload.id) {
        Some(val) => ConfigureTemplate {
            title: val.title.to_string(),
            alternate: val.alternate.to_string(),
            id: val.id,
            season: val.season,
            source: val.source.to_string(),
        },
        None => {
            let lock = state.lock().await;
            let show = lock.tracker.get(&payload.id).unwrap();
            ConfigureTemplate {
                title: show.title.to_string(),
                alternate: show.title.to_string(),
                id: payload.id,
                season: 1,
                source: "subsplease".into(),
            }
        }
    };

    HtmlTemplate::new(template)
}

#[axum::debug_handler]
pub async fn save_configuration(
    Form(payload): Form<TrackerDataEntry>
) -> impl IntoResponse {
    let mut data = read_tracked_data().await.unwrap();
    data.insert(payload.id, payload);
    match save_tracked_data(&data).await {
        anyhow::Result::Ok(_) => {},
        Err(err) => { println!("{:?}", err); }
    };
}


#[axum::debug_handler]
pub async fn close() -> impl IntoResponse {
    Html("")
}








