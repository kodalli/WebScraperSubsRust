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

#[derive(Deserialize)]
pub struct UserState {
    pub user: String,
    pub tracker: HashMap<String, bool>,
}

#[derive(Deserialize)]
pub struct TrackerQuery {
    pub title: String,
}

impl UserState {
    pub fn new(user: String) -> Self {
        Self {
            user,
            tracker: HashMap::new(),
        }
    }
}

#[derive(Template)]
#[template(path = "pages/home.html")]
pub struct HomeTemplate {
    pub user: String,
    pub shows: Vec<AniShow>,
}

async fn get_seasonal() -> anyhow::Result<Vec<AniShow>> {
    match get_anilist_data(Season::FALL, 2023).await {
        std::result::Result::Ok(res) => Ok(res),
        std::result::Result::Err(err) => {
            println!("Failed to fetch seasonal anime. Error: {}", err);
            Ok(Vec::new())
        }
    }
}

pub async fn view(State(state): State<Arc<Mutex<UserState>>>) -> impl IntoResponse {
    let user = { state.lock().await.user.clone() };
    let cards: Vec<AniShow> = get_seasonal().await.unwrap_or_default();
    let template = HomeTemplate { user, shows: cards };
    HtmlTemplate::new(template)
}

#[axum::debug_handler]
pub async fn update_user(
    State(state): State<Arc<Mutex<UserState>>>,
    Form(payload): Form<UserState>,
) -> impl IntoResponse {
    // Lock the mutex to get mutable access
    let mut lock = state.lock().await;

    // Update state
    *lock = UserState::new(payload.user.to_string());

    Html(format!("{}", payload.user))
}

#[axum::debug_handler]
pub async fn set_tracker(
    State(state): State<Arc<Mutex<UserState>>>,
    Form(payload): Form<TrackerQuery>,
) -> impl IntoResponse {
    let title = payload.title.clone();
    println!("Set Tracker! {:?}", title);
    let mut lock = state.lock().await;

    // Using the entry API for more idiomatic code
    let status = match lock.tracker.entry(title) {
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

    drop(lock); // Explicitly drop the lock before potentially awaiting again

    if status {
        Html("Tracking")
    } else {
        Html("Not Tracked")
    }
}

#[axum::debug_handler]
pub async fn get_tracker(
    State(state): State<Arc<Mutex<UserState>>>,
    Query(query): Query<TrackerQuery>,
) -> impl IntoResponse {
    println!("Get Tracker! {:?}", &query.title);
    let lock = state.lock().await;

    let status = lock.tracker.get(&query.title).unwrap_or(&false);

    if *status {
        Html("Tracking")
    } else {
        Html("Not Tracked")
    }
}
