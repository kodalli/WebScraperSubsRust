use crate::{
    pages::{filters, HtmlTemplate},
    scraper::anilist::{get_anilist_data, AniShow, Season},
};
use anyhow::Ok;
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
    Form,
};
use chrono::{Datelike, Utc};
use core::fmt;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Deserialize)]
pub struct UserState {
    pub user: String,
    pub tracker: HashMap<String, bool>,
    pub season: Season,
    pub year: u16,
}

#[derive(Deserialize)]
pub struct TrackerQuery {
    pub title: String,
}

#[derive(Deserialize)]
pub struct SeasonalAnimeQuery {
    pub season: String,
    pub year: u16,
}

impl UserState {
    pub fn new(user: String) -> Self {
        let (season, year) = current_year_and_season();
        Self {
            user,
            tracker: HashMap::new(),
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
    pub season: Season,
    pub year: u16,
}

#[derive(Template)]
#[template(path = "components/card.html")]
pub struct CardTemplate {
    pub show: AniShow,
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

async fn get_seasonal(season: Season, year: u16) -> anyhow::Result<Vec<AniShow>> {
    match get_anilist_data(season, year).await {
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

pub async fn view(State(state): State<Arc<Mutex<UserState>>>) -> impl IntoResponse {
    let lock = state.lock().await;
    let shows: Vec<AniShow> = get_seasonal(lock.season, lock.year)
        .await
        .unwrap_or_default();
    let card_templates: Vec<CardTemplate> = shows
        .iter()
        .map(|show| CardTemplate { show: show.clone() })
        .collect();
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
    for c in &cards {
        if c.title
            .as_ref()
            .unwrap()
            .romaji
            .as_ref()
            .unwrap()
            .contains("Baki")
        {
            println!("");
            println!(
                "show: {}",
                c.title.as_ref().unwrap().romaji.as_ref().unwrap()
            );
            println!("");
            println!("description: {}", c.description.as_ref().unwrap());
            println!("");
        }
    }
    let card_templates: Vec<CardTemplate> = cards
        .iter()
        .map(|show| CardTemplate { show: show.clone() })
        .collect();

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

#[axum::debug_handler]
pub async fn set_tracker(
    State(state): State<Arc<Mutex<UserState>>>,
    Form(payload): Form<TrackerQuery>,
) -> impl IntoResponse {
    let title = payload.title;
    println!("Set Tracker! {:?}", title);
    let mut lock = state.lock().await;

    // Using the entry API for more idiomatic code
    let is_tracking = match lock.tracker.entry(title.clone()) {
        std::collections::hash_map::Entry::Vacant(e) => {
            e.insert(true);
            true
        }
        std::collections::hash_map::Entry::Occupied(mut e) => {
            let current_status = *e.get();
            *e.get_mut() = !current_status;
            !current_status
        }
    };

    render_tracking(is_tracking, &title)
}

#[axum::debug_handler]
pub async fn get_tracker(
    State(state): State<Arc<Mutex<UserState>>>,
    Query(query): Query<TrackerQuery>,
) -> impl IntoResponse {
    println!("Get Tracker! {:?}", &query.title);
    let lock = state.lock().await;

    let is_tracking = lock.tracker.get(&query.title).unwrap_or(&false);

    render_tracking(*is_tracking, &query.title)
}

fn render_tracking(status: bool, title: &str) -> impl IntoResponse {
    let not_tracked = "rounded-md bg-yellow-400 px-2 py-3 font-semibold text-black shadow-sm transition-colors hover:bg-black hover:text-white hover:ring-2 hover:ring-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-white";
    let tracked = "rounded-md bg-gray-900 px-2 py-3 font-semibold text-white shadow-sm transition-colors hover:bg-white hover:text-black hover:ring-2 hover:ring-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-black";
    if status {
        Html(render_tracking_button("Tracking", tracked, title))
    } else {
        Html(render_tracking_button("Not Tracked", not_tracked, title))
    }
}

fn render_tracking_button(status: &str, class: &str, title: &str) -> String {
    format!(
        r#"<button class="text-right text-xs px-2 py-1 rounded {}" hx-post="/api/set_tracker"
        hx-vals='{{"title": "{}"}}' hx-swap="outerHTML">
        <span>{}</span>
    </button>"#,
        class, title.replace("\"", "\\\""), status
    )
}
