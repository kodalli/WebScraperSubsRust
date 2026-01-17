use crate::{
    db::{self, models::Show},
    pages::{filters, HtmlTemplate},
    scraper::{
        anilist::{get_anilist_all_airing, get_anilist_data, AniShow, NextAiringEpisode, Season},
        nyaasi::{fetch_sources, Link},
        rss::{fetch_rss_feed, parse_episode_info},
        season_parser::detect_season,
        transmission::{clear_all_torrents, upload_to_transmission_rpc},
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

/// Represents a potential match from RSS/nyaasi search
#[derive(Debug, Clone, Serialize)]
pub struct MatchCandidate {
    pub show_title: String,
    pub episode_count: u16,
    pub quality: String,
    pub latest_episode: u16,
}

#[derive(Template)]
#[template(path = "components/match_selection.html")]
pub struct MatchSelectionTemplate {
    pub original_title: String,
    pub show_id: u32,
    pub matches: Vec<MatchCandidate>,
    pub source: String,
    pub fallback_available: bool,
    pub latest_episode: String,
    pub next_air_date: String,
}

#[derive(Deserialize)]
pub struct SearchMatchesQuery {
    pub id: u32,
    pub title: String,
    pub source: String,
    pub latest_episode: String,
    pub next_air_date: String,
}

#[derive(Deserialize)]
pub struct ConfirmMatchQuery {
    pub id: u32,
    pub title: String,
    pub alternate: String,
    pub latest_episode: String,
    pub next_air_date: String,
}

#[derive(Deserialize)]
pub struct SkipMatchQuery {
    pub id: u32,
    pub title: String,
    pub latest_episode: String,
    pub next_air_date: String,
}

/// Empty template for returning just headers
#[derive(Template)]
#[template(source = "", ext = "html")]
pub struct EmptyTemplate;

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

    // If untracking, just update and return
    if !new_payload.is_tracked {
        lock.tracker.remove(&new_payload.id);

        let show_id = new_payload.id;
        let latest_episode = new_payload.latest_episode.clone();
        let next_air_date = new_payload.next_air_date.clone();

        let db_result = db::with_db(move |conn| {
            if let Some(mut existing_show) = db::get_show(conn, show_id)? {
                existing_show.is_tracked = false;
                existing_show.latest_episode = Some(latest_episode);
                existing_show.next_air_date = Some(next_air_date);
                db::update_show(conn, &existing_show)?;
            }
            Ok(())
        })
        .await;

        if let Err(err) = db_result {
            eprintln!("Could not save to database: {:?}", err);
        }

        let template = TrackedTemplate { entry: new_payload };
        return HtmlTemplate::new(template)
            .with_header("HX-Trigger", "newTrackerStatus")
            .into_response();
    }

    // When tracking, search for matches first
    // Drop the lock before async operations
    drop(lock);

    let title = new_payload.title.clone();

    // Search SubsPlease RSS first
    let matches = search_rss_matches("subsplease", &title).await;

    // Check for exact match
    if let Some(exact_match) = has_exact_match(&title, &matches) {
        // Exact match found - save directly with matched title as alternate
        let mut lock = state.lock().await;
        lock.tracker.insert(new_payload.id, new_payload.clone());

        // Auto-detect season from the matched title
        let season_info = detect_season(&exact_match);
        let detected_season = season_info.season;

        let show_id = new_payload.id;
        let orig_title = new_payload.title.clone();
        let latest_episode = new_payload.latest_episode.clone();
        let next_air_date = new_payload.next_air_date.clone();

        let db_result = db::with_db(move |conn| {
            if let Some(mut existing_show) = db::get_show(conn, show_id)? {
                existing_show.is_tracked = true;
                existing_show.alternate = exact_match;
                existing_show.season = detected_season;
                existing_show.latest_episode = Some(latest_episode);
                existing_show.next_air_date = Some(next_air_date);
                db::update_show(conn, &existing_show)?;
            } else {
                let new_show = Show {
                    id: show_id,
                    title: orig_title,
                    alternate: exact_match,
                    season: detected_season,
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
        return HtmlTemplate::new(template)
            .with_header("HX-Trigger", "newTrackerStatus")
            .into_response();
    }

    // No exact match - check if we have partial matches or need to search Nyaa.si
    if !matches.is_empty() {
        // Show modal with SubsPlease matches
        let template = MatchSelectionTemplate {
            original_title: new_payload.title.clone(),
            show_id: new_payload.id,
            matches,
            source: "subsplease".to_string(),
            fallback_available: true,
            latest_episode: new_payload.latest_episode.clone(),
            next_air_date: new_payload.next_air_date.clone(),
        };

        return HtmlTemplate::new(template)
            .with_header("HX-Retarget", "#configuration-modal")
            .with_header("HX-Reswap", "innerHTML")
            .into_response();
    }

    // No SubsPlease results - try Nyaa.si
    let nyaasi_matches = search_nyaasi_matches(&title).await;

    if !nyaasi_matches.is_empty() {
        // Check for exact match in Nyaa.si results
        if let Some(exact_match) = has_exact_match(&title, &nyaasi_matches) {
            let mut lock = state.lock().await;
            lock.tracker.insert(new_payload.id, new_payload.clone());

            // Auto-detect season from the matched title
            let season_info = detect_season(&exact_match);
            let detected_season = season_info.season;

            let show_id = new_payload.id;
            let orig_title = new_payload.title.clone();
            let latest_episode = new_payload.latest_episode.clone();
            let next_air_date = new_payload.next_air_date.clone();

            let db_result = db::with_db(move |conn| {
                if let Some(mut existing_show) = db::get_show(conn, show_id)? {
                    existing_show.is_tracked = true;
                    existing_show.alternate = exact_match;
                    existing_show.season = detected_season;
                    existing_show.latest_episode = Some(latest_episode);
                    existing_show.next_air_date = Some(next_air_date);
                    db::update_show(conn, &existing_show)?;
                } else {
                    let new_show = Show {
                        id: show_id,
                        title: orig_title,
                        alternate: exact_match,
                        season: detected_season,
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
            return HtmlTemplate::new(template)
                .with_header("HX-Trigger", "newTrackerStatus")
                .into_response();
        }

        // Show modal with Nyaa.si matches
        let template = MatchSelectionTemplate {
            original_title: new_payload.title.clone(),
            show_id: new_payload.id,
            matches: nyaasi_matches,
            source: "nyaasi".to_string(),
            fallback_available: false,
            latest_episode: new_payload.latest_episode.clone(),
            next_air_date: new_payload.next_air_date.clone(),
        };

        return HtmlTemplate::new(template)
            .with_header("HX-Retarget", "#configuration-modal")
            .with_header("HX-Reswap", "innerHTML")
            .into_response();
    }

    // No results anywhere - save with original title
    let mut lock = state.lock().await;
    lock.tracker.insert(new_payload.id, new_payload.clone());

    // Auto-detect season from the title
    let season_info = detect_season(&new_payload.title);
    let detected_season = season_info.season;

    let show_id = new_payload.id;
    let title = new_payload.title.clone();
    let latest_episode = new_payload.latest_episode.clone();
    let next_air_date = new_payload.next_air_date.clone();

    let db_result = db::with_db(move |conn| {
        if let Some(mut existing_show) = db::get_show(conn, show_id)? {
            existing_show.is_tracked = true;
            existing_show.season = detected_season;
            existing_show.latest_episode = Some(latest_episode);
            existing_show.next_air_date = Some(next_air_date);
            db::update_show(conn, &existing_show)?;
        } else {
            let new_show = Show {
                id: show_id,
                title: title.clone(),
                alternate: title,
                season: detected_season,
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
    HtmlTemplate::new(template)
        .with_header("HX-Trigger", "newTrackerStatus")
        .into_response()
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

#[axum::debug_handler]
pub async fn sync_now() -> impl IntoResponse {
    use crate::scraper::tracker::download_shows;

    match download_shows().await {
        Ok(_) => Html("<span class=\"text-green-400\">Sync complete!</span>".to_string()),
        Err(e) => {
            eprintln!("Sync failed: {:?}", e);
            Html("<span class=\"text-red-400\">Sync failed</span>".to_string())
        }
    }
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

/// Clear all torrents from Transmission and delete their files
#[axum::debug_handler]
pub async fn clear_transmission() -> impl IntoResponse {
    match clear_all_torrents(true).await {
        Ok(count) => {
            Html(format!(
                "<span class=\"text-green-400\">Removed {} torrent(s) and deleted files</span>",
                count
            ))
        }
        Err(e) => {
            eprintln!("Failed to clear Transmission: {:?}", e);
            Html(format!(
                "<span class=\"text-red-400\">Failed: {}</span>",
                e
            ))
        }
    }
}

// ============================================================================
// Filter Management API Endpoints
// ============================================================================

/// Get all global filters
#[axum::debug_handler]
pub async fn get_filters() -> impl IntoResponse {
    match db::with_db(|conn| db::get_all_filters(conn)).await {
        Ok(filters) => Json(filters).into_response(),
        Err(err) => {
            eprintln!("Failed to get filters: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get filters",
            )
                .into_response()
        }
    }
}

/// Search RSS feed and aggregate results by show title
async fn search_rss_matches(source: &str, title: &str) -> Vec<MatchCandidate> {
    let rss_items = match fetch_rss_feed(source, title).await {
        Ok(items) => items,
        Err(e) => {
            eprintln!("Failed to fetch RSS feed for '{}': {:?}", title, e);
            return Vec::new();
        }
    };

    // Group by show title
    let mut show_map: HashMap<String, (u16, u16, String)> = HashMap::new(); // (count, latest_episode, quality)

    for item in &rss_items {
        if let Some((show_title, episode, quality)) = parse_episode_info(&item.title) {
            let entry = show_map.entry(show_title).or_insert((0, 0, quality.clone()));
            entry.0 += 1; // increment count
            if episode > entry.1 {
                entry.1 = episode; // track latest episode
            }
            if entry.2.is_empty() {
                entry.2 = quality;
            }
        }
    }

    show_map
        .into_iter()
        .map(|(show_title, (episode_count, latest_episode, quality))| MatchCandidate {
            show_title,
            episode_count,
            latest_episode,
            quality,
        })
        .collect()
}

/// Search Nyaa.si HTTP and aggregate results by show title
async fn search_nyaasi_matches(title: &str) -> Vec<MatchCandidate> {
    let links = match fetch_sources(title, "default").await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to fetch Nyaa.si for '{}': {:?}", title, e);
            return Vec::new();
        }
    };

    // Group by show title
    let mut show_map: HashMap<String, (u16, u16)> = HashMap::new(); // (count, latest_episode)

    for link in &links {
        let episode: u16 = link.episode.parse().unwrap_or(0);
        let entry = show_map.entry(link.title.clone()).or_insert((0, 0));
        entry.0 += 1;
        if episode > entry.1 {
            entry.1 = episode;
        }
    }

    show_map
        .into_iter()
        .map(|(show_title, (episode_count, latest_episode))| MatchCandidate {
            show_title,
            episode_count,
            latest_episode,
            quality: "1080p".to_string(), // Nyaa.si results are pre-filtered to 1080p
        })
        .collect()
}

/// Check if any match equals the title (case-insensitive)
fn has_exact_match(title: &str, matches: &[MatchCandidate]) -> Option<String> {
    let title_lower = title.to_lowercase();
    matches
        .iter()
        .find(|m| m.show_title.to_lowercase() == title_lower)
        .map(|m| m.show_title.clone())
}

/// Handler to search for matches (called when user clicks "Search Nyaa.si Instead")
#[axum::debug_handler]
pub async fn search_matches(Query(payload): Query<SearchMatchesQuery>) -> impl IntoResponse {
    let matches = if payload.source == "nyaasi" {
        search_nyaasi_matches(&payload.title).await
    } else {
        search_rss_matches(&payload.source, &payload.title).await
    };

    let fallback_available = payload.source != "nyaasi";

    let template = MatchSelectionTemplate {
        original_title: payload.title,
        show_id: payload.id,
        matches,
        source: payload.source,
        fallback_available,
        latest_episode: payload.latest_episode,
        next_air_date: payload.next_air_date,
    };

    HtmlTemplate::new(template)
}

/// Save show with selected alternate name
#[axum::debug_handler]
pub async fn confirm_match(
    State(state): State<Arc<Mutex<UserState>>>,
    Query(payload): Query<ConfirmMatchQuery>,
) -> impl IntoResponse {
    let mut lock = state.lock().await;

    // Update in-memory state
    let entry = TableEntry {
        title: payload.title.clone(),
        latest_episode: payload.latest_episode.clone(),
        next_air_date: payload.next_air_date.clone(),
        is_tracked: true,
        id: payload.id,
    };
    lock.tracker.insert(payload.id, entry);

    // Auto-detect season from the alternate title
    let season_info = detect_season(&payload.alternate);
    let detected_season = season_info.season;

    // Save to database with the selected alternate name
    let show_id = payload.id;
    let title = payload.title.clone();
    let alternate = payload.alternate.clone();
    let latest_episode = payload.latest_episode.clone();
    let next_air_date = payload.next_air_date.clone();

    let db_result = db::with_db(move |conn| {
        if let Some(mut existing_show) = db::get_show(conn, show_id)? {
            existing_show.is_tracked = true;
            existing_show.alternate = alternate;
            existing_show.season = detected_season;
            existing_show.latest_episode = Some(latest_episode);
            existing_show.next_air_date = Some(next_air_date);
            db::update_show(conn, &existing_show)?;
        } else {
            let new_show = Show {
                id: show_id,
                title,
                alternate,
                season: detected_season,
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

    // Return empty HTML with trigger to update the tracker table
    HtmlTemplate::new(EmptyTemplate).with_header("HX-Trigger", "newTrackerStatus")
}

/// Skip match selection - track with original title
#[axum::debug_handler]
pub async fn skip_match_selection(
    State(state): State<Arc<Mutex<UserState>>>,
    Query(payload): Query<SkipMatchQuery>,
) -> impl IntoResponse {
    let mut lock = state.lock().await;

    // Update in-memory state
    let entry = TableEntry {
        title: payload.title.clone(),
        latest_episode: payload.latest_episode.clone(),
        next_air_date: payload.next_air_date.clone(),
        is_tracked: true,
        id: payload.id,
    };
    lock.tracker.insert(payload.id, entry);

    // Auto-detect season from the title
    let season_info = detect_season(&payload.title);
    let detected_season = season_info.season;

    // Save to database with original title as alternate
    let show_id = payload.id;
    let title = payload.title.clone();
    let latest_episode = payload.latest_episode.clone();
    let next_air_date = payload.next_air_date.clone();

    let db_result = db::with_db(move |conn| {
        if let Some(mut existing_show) = db::get_show(conn, show_id)? {
            existing_show.is_tracked = true;
            existing_show.season = detected_season;
            existing_show.latest_episode = Some(latest_episode);
            existing_show.next_air_date = Some(next_air_date);
            db::update_show(conn, &existing_show)?;
        } else {
            let new_show = Show {
                id: show_id,
                title: title.clone(),
                alternate: title,
                season: detected_season,
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

    // Return empty HTML with trigger to update the tracker table
    HtmlTemplate::new(EmptyTemplate).with_header("HX-Trigger", "newTrackerStatus")
}

/// Form data for creating a filter
#[derive(Debug, Deserialize)]
pub struct CreateFilterForm {
    pub name: String,
    pub filter_type: String,
    pub pattern: String,
    pub action: String,
    #[serde(default)]
    pub priority: i32,
}

/// Create a new filter
#[axum::debug_handler]
pub async fn create_filter(Form(payload): Form<CreateFilterForm>) -> impl IntoResponse {
    let filter = db::CreateFilterRule {
        name: payload.name,
        filter_type: payload.filter_type,
        pattern: payload.pattern,
        action: payload.action,
        priority: payload.priority,
    };

    match db::with_db(move |conn| db::create_filter(conn, &filter)).await {
        Ok(id) => Json(serde_json::json!({"status": "ok", "id": id})).into_response(),
        Err(err) => {
            eprintln!("Failed to create filter: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create filter",
            )
                .into_response()
        }
    }
}

/// Path parameter for filter ID
#[derive(Debug, Deserialize)]
pub struct FilterIdPath {
    pub id: u32,
}

/// Form data for updating a filter
#[derive(Debug, Deserialize)]
pub struct UpdateFilterForm {
    pub name: Option<String>,
    pub filter_type: Option<String>,
    pub pattern: Option<String>,
    pub action: Option<String>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
}

/// Update an existing filter
#[axum::debug_handler]
pub async fn update_filter(
    axum::extract::Path(path): axum::extract::Path<FilterIdPath>,
    Form(payload): Form<UpdateFilterForm>,
) -> impl IntoResponse {
    let update = db::UpdateFilterRule {
        name: payload.name,
        filter_type: payload.filter_type,
        pattern: payload.pattern,
        action: payload.action,
        priority: payload.priority,
        enabled: payload.enabled,
    };

    let filter_id = path.id;
    match db::with_db(move |conn| db::update_filter(conn, filter_id, &update)).await {
        Ok(updated) => {
            if updated {
                Json(serde_json::json!({"status": "ok"})).into_response()
            } else {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    "Filter not found",
                )
                    .into_response()
            }
        }
        Err(err) => {
            eprintln!("Failed to update filter: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update filter",
            )
                .into_response()
        }
    }
}

/// Delete a filter
#[axum::debug_handler]
pub async fn delete_filter(
    axum::extract::Path(path): axum::extract::Path<FilterIdPath>,
) -> impl IntoResponse {
    let filter_id = path.id;
    match db::with_db(move |conn| db::delete_filter(conn, filter_id)).await {
        Ok(deleted) => {
            if deleted {
                Json(serde_json::json!({"status": "ok"})).into_response()
            } else {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    "Filter not found",
                )
                    .into_response()
            }
        }
        Err(err) => {
            eprintln!("Failed to delete filter: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete filter",
            )
                .into_response()
        }
    }
}

/// Toggle a filter's enabled status
#[axum::debug_handler]
pub async fn toggle_filter(
    axum::extract::Path(path): axum::extract::Path<FilterIdPath>,
) -> impl IntoResponse {
    let filter_id = path.id;
    match db::with_db(move |conn| db::toggle_filter(conn, filter_id)).await {
        Ok(toggled) => {
            if toggled {
                Json(serde_json::json!({"status": "ok"})).into_response()
            } else {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    "Filter not found",
                )
                    .into_response()
            }
        }
        Err(err) => {
            eprintln!("Failed to toggle filter: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to toggle filter",
            )
                .into_response()
        }
    }
}

/// Path parameter for show ID (for show-specific filters)
#[derive(Debug, Deserialize)]
pub struct ShowIdPath {
    pub show_id: u32,
}

/// Get show-specific filters
#[axum::debug_handler]
pub async fn get_show_filters(
    axum::extract::Path(path): axum::extract::Path<ShowIdPath>,
) -> impl IntoResponse {
    let show_id = path.show_id;
    match db::with_db(move |conn| db::get_show_filters(conn, show_id)).await {
        Ok(filters) => Json(filters).into_response(),
        Err(err) => {
            eprintln!("Failed to get show filters: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get show filters",
            )
                .into_response()
        }
    }
}

/// Form data for creating a show-specific filter
#[derive(Debug, Deserialize)]
pub struct CreateShowFilterForm {
    pub filter_rule_id: Option<u32>,
    pub filter_type: Option<String>,
    pub pattern: Option<String>,
    pub action: String,
}

/// Create a show-specific filter
#[axum::debug_handler]
pub async fn create_show_filter(
    axum::extract::Path(path): axum::extract::Path<ShowIdPath>,
    Form(payload): Form<CreateShowFilterForm>,
) -> impl IntoResponse {
    let show_id = path.show_id;
    let filter_rule_id = payload.filter_rule_id;
    let filter_type = payload.filter_type;
    let pattern = payload.pattern;
    let action = payload.action;

    match db::with_db(move |conn| {
        db::create_show_filter(
            conn,
            show_id,
            filter_rule_id,
            filter_type.as_deref(),
            pattern.as_deref(),
            &action,
        )
    })
    .await
    {
        Ok(id) => Json(serde_json::json!({"status": "ok", "id": id})).into_response(),
        Err(err) => {
            eprintln!("Failed to create show filter: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create show filter",
            )
                .into_response()
        }
    }
}

/// Path parameter for show filter ID
#[derive(Debug, Deserialize)]
pub struct ShowFilterIdPath {
    pub show_id: u32,
    pub filter_id: u32,
}

/// Delete a show-specific filter
#[axum::debug_handler]
pub async fn delete_show_filter(
    axum::extract::Path(path): axum::extract::Path<ShowFilterIdPath>,
) -> impl IntoResponse {
    let filter_id = path.filter_id;
    match db::with_db(move |conn| db::delete_show_filter(conn, filter_id)).await {
        Ok(deleted) => {
            if deleted {
                Json(serde_json::json!({"status": "ok"})).into_response()
            } else {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    "Show filter not found",
                )
                    .into_response()
            }
        }
        Err(err) => {
            eprintln!("Failed to delete show filter: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete show filter",
            )
                .into_response()
        }
    }
}








