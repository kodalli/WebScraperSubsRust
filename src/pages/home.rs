use crate::{
    db::{self, models::Show},
    pages::{filters, HtmlTemplate},
    scraper::{
        anilist::{get_anilist_all_airing, get_anilist_data, AniShow, NextAiringEpisode, Season},
        nyaasi::{fetch_sources, Link},
        transmission::upload_to_transmission_rpc,
    },
};
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Json},
    Form,
};
use chrono::{DateTime, Datelike, Utc};
use core::fmt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

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
    #[serde(default = "default_quality")]
    pub quality: String,
    pub download_path: Option<String>,
    #[serde(default)]
    pub last_downloaded_episode: u16,
}

fn default_quality() -> String {
    "1080p".to_string()
}

#[derive(Deserialize)]
pub struct RssConfigForm {
    pub poll_times_per_day: u8,
    pub enabled: bool,
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
    pub quality: String,
    pub download_path: Option<String>,
    pub last_downloaded_episode: u16,
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
        |d| match DateTime::from_timestamp(d, 0) {
            Some(date) => format!("{}", date.naive_utc()),
            None => "N/A".into(),
        },
    );

    (episode, air_date)
}

fn calculate_sort_score(show: &AniShow) -> f64 {
    let rating = show.average_score.or(show.mean_score);
    let popularity = show.popularity.unwrap_or(0);

    match rating {
        Some(r) => {
            let norm_rating = r as f64 / 100.0;
            let norm_pop = if popularity > 0 {
                (popularity as f64).log10() / 7.0
            } else {
                0.0
            };
            (0.7 * norm_rating) + (0.3 * norm_pop)
        }
        None => {
            // No rating - use popularity only
            if popularity > 0 {
                (popularity as f64).log10() / 7.0 * 0.3
            } else {
                0.0
            }
        }
    }
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
    let mut shows: Vec<AniShow> = get_seasonal(lock.season, lock.year)
        .await
        .unwrap_or_default();
    shows.sort_by(|a, b| {
        calculate_sort_score(b)
            .partial_cmp(&calculate_sort_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
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

    let mut cards: Vec<AniShow> = get_seasonal(lock.season, lock.year)
        .await
        .unwrap_or_default();
    cards.sort_by(|a, b| {
        calculate_sort_score(b)
            .partial_cmp(&calculate_sort_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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
    let mut cards: Vec<AniShow> = get_currently_airing().await.unwrap_or_default();
    cards.sort_by(|a, b| {
        calculate_sort_score(b)
            .partial_cmp(&calculate_sort_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
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

/// Load tracked shows from the SQLite database into a HashMap
pub async fn read_tracked_shows() -> anyhow::Result<HashMap<u32, TableEntry>> {
    let shows = db::with_db(|conn| db::get_tracked_shows(conn)).await?;
    let map = shows
        .into_iter()
        .map(|show| {
            (
                show.id,
                TableEntry {
                    title: show.title,
                    latest_episode: show.latest_episode.unwrap_or_else(|| "N/A".to_string()),
                    next_air_date: show.next_air_date.unwrap_or_else(|| "N/A".to_string()),
                    is_tracked: show.is_tracked,
                    id: show.id,
                },
            )
        })
        .collect();
    Ok(map)
}

#[axum::debug_handler]
pub async fn set_tracker(
    State(state): State<Arc<Mutex<UserState>>>,
    Query(payload): Query<TableEntry>,
) -> impl IntoResponse {
    let mut lock = state.lock().await;
    let mut new_payload = payload.clone();
    new_payload.is_tracked = !new_payload.is_tracked;

    // Update in-memory state
    if new_payload.is_tracked {
        lock.tracker.insert(new_payload.id, new_payload.clone());
    } else {
        lock.tracker.remove(&new_payload.id);
    }

    // Save to database
    let show_id = new_payload.id;
    let is_tracked = new_payload.is_tracked;
    let title = new_payload.title.clone();
    let latest_episode = new_payload.latest_episode.clone();
    let next_air_date = new_payload.next_air_date.clone();

    let db_result = db::with_db(move |conn| {
        // Check if show exists in database
        if let Some(mut existing_show) = db::get_show(conn, show_id)? {
            // Update existing show's tracked status
            existing_show.is_tracked = is_tracked;
            existing_show.latest_episode = Some(latest_episode.clone());
            existing_show.next_air_date = Some(next_air_date.clone());
            db::update_show(conn, &existing_show)?;
        } else if is_tracked {
            // Insert new show if it's being tracked
            let new_show = Show {
                id: show_id,
                title,
                alternate: String::new(),
                season: 1,
                source: "subsplease".to_string(),
                quality: "1080p".to_string(),
                download_path: None,
                last_downloaded_episode: 0,
                last_downloaded_hash: None,
                is_tracked: true,
                latest_episode: Some(latest_episode),
                next_air_date: Some(next_air_date),
                created_at: None,
                updated_at: None,
            };
            db::insert_show(conn, &new_show)?;
        }
        Ok(())
    })
    .await;

    if let Err(err) = db_result {
        eprintln!("Could not save to database: {:?}", err);
    }

    let template = TrackedTemplate { entry: new_payload };
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


#[axum::debug_handler]
pub async fn get_configuration(
    State(state): State<Arc<Mutex<UserState>>>,
    Query(payload): Query<AnimeIdQuery>,
) -> impl IntoResponse {
    let show_id = payload.id;
    let db_show = db::with_db(move |conn| db::get_show(conn, show_id)).await;

    let template = match db_show {
        Ok(Some(show)) => ConfigureTemplate {
            title: show.title,
            alternate: show.alternate,
            id: show.id,
            season: show.season,
            source: show.source,
            quality: show.quality,
            download_path: show.download_path,
            last_downloaded_episode: show.last_downloaded_episode,
        },
        _ => {
            // Fall back to in-memory tracker if not in database
            let lock = state.lock().await;
            if let Some(show) = lock.tracker.get(&payload.id) {
                ConfigureTemplate {
                    title: show.title.to_string(),
                    alternate: show.title.to_string(),
                    id: payload.id,
                    season: 1,
                    source: "subsplease".into(),
                    quality: "1080p".into(),
                    download_path: None,
                    last_downloaded_episode: 0,
                }
            } else {
                // Default template for unknown show
                ConfigureTemplate {
                    title: "Unknown".into(),
                    alternate: "Unknown".into(),
                    id: payload.id,
                    season: 1,
                    source: "subsplease".into(),
                    quality: "1080p".into(),
                    download_path: None,
                    last_downloaded_episode: 0,
                }
            }
        }
    };

    HtmlTemplate::new(template)
}

#[axum::debug_handler]
pub async fn save_configuration(Form(payload): Form<TrackerDataEntry>) -> impl IntoResponse {
    let show_id = payload.id;
    let title = payload.title.clone();
    let alternate = payload.alternate.clone();
    let season = payload.season;
    let source = payload.source.clone();
    let quality = payload.quality.clone();
    let download_path = payload.download_path.clone();

    let db_result = db::with_db(move |conn| {
        // Check if show exists
        if let Some(mut existing_show) = db::get_show(conn, show_id)? {
            // Update existing show's configuration
            existing_show.alternate = alternate;
            existing_show.season = season;
            existing_show.source = source;
            existing_show.quality = quality;
            existing_show.download_path = download_path;
            db::update_show(conn, &existing_show)?;
        } else {
            // Insert new show
            let new_show = Show {
                id: show_id,
                title,
                alternate,
                season,
                source,
                quality,
                download_path,
                last_downloaded_episode: 0,
                last_downloaded_hash: None,
                is_tracked: true,
                latest_episode: None,
                next_air_date: None,
                created_at: None,
                updated_at: None,
            };
            db::insert_show(conn, &new_show)?;
        }
        Ok(())
    })
    .await;

    if let Err(err) = db_result {
        eprintln!("Could not save configuration to database: {:?}", err);
    }

    Html("")
}


#[axum::debug_handler]
pub async fn close() -> impl IntoResponse {
    Html("")
}

/// Get the current RSS configuration
#[axum::debug_handler]
pub async fn get_rss_config() -> impl IntoResponse {
    match db::with_db(|conn| db::get_rss_config(conn)).await {
        Ok(config) => Json(config).into_response(),
        Err(err) => {
            eprintln!("Failed to get RSS config: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get RSS config",
            )
                .into_response()
        }
    }
}

/// Save the RSS configuration
#[axum::debug_handler]
pub async fn save_rss_config(Form(payload): Form<RssConfigForm>) -> impl IntoResponse {
    let poll_times = payload.poll_times_per_day;
    let enabled = payload.enabled;

    let result = db::with_db(move |conn| {
        db::update_poll_interval(conn, poll_times)?;
        db::set_rss_enabled(conn, enabled)
    })
    .await;

    match result {
        Ok(_) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(err) => {
            eprintln!("Failed to save RSS config: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to save RSS config",
            )
                .into_response()
        }
    }
}








