use crate::{
    pages::{filters, HtmlTemplate},
    scraper::anilist::{get_anilist_data, AniShow, Season},
};
use anyhow::Ok;
use askama::Template;
use axum::{
    extract::{State, Query},
    response::{Html, IntoResponse},
    Form,
};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use chrono::{Datelike, Utc};

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
    pub season: Season,
    pub year: u16,
}

impl UserState {
    pub fn new(user: String) -> Self {
        let (season, year) = current_year_and_season();
        Self {
            user,
            tracker: HashMap::new(),
            season,
            year
        }
    }
}

#[derive(Template)]
#[template(path = "pages/home.html")]
pub struct HomeTemplate {
    pub user: String,
    pub shows: Vec<AniShow>,
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

pub async fn view(State(state): State<Arc<Mutex<UserState>>>) -> impl IntoResponse {
    let lock = state.lock().await;
    let UserState { user, season, year, .. } = &*lock;
    let cards: Vec<AniShow> = get_seasonal(*season, *year).await.unwrap_or_default();
    let template = HomeTemplate { user: user.clone(), shows: cards };
    drop(lock);
    HtmlTemplate::new(template)
}

#[axum::debug_handler]
pub async fn navigate_seasonal_anime(
    State(state): State<Arc<Mutex<UserState>>>,
    Form(payload): Form<SeasonalAnimeQuery>,
) -> impl IntoResponse {
    // Lock the mutex to get mutable access
    let mut lock = state.lock().await;
    lock.season = payload.season;
    lock.year = payload.year;

    Html("")
}


#[axum::debug_handler]
pub async fn update_user(
    State(state): State<Arc<Mutex<UserState>>>,
    Form(payload): Form<UserState>,
) -> impl IntoResponse {
    // Lock the mutex to get mutable access
    let mut lock = state.lock().await;

    // Update state
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
        class, title, status
    )
}
