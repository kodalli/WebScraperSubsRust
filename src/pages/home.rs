use crate::{
    pages::HtmlTemplate,
    scraper::anilist::{get_anilist_data, AniShow, Season},
};
use anyhow::Ok;
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    Form,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Deserialize)]
pub struct UserState {
    user: String,
}

impl UserState {
    pub fn new(user: String) -> Self {
        Self { user }
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
