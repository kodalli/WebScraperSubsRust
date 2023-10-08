use crate::pages::HtmlTemplate;
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse}, Form,
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
}

pub async fn view(State(state): State<Arc<Mutex<UserState>>>) -> impl IntoResponse {
    let lock = state.lock().await;
    let template = HomeTemplate {
        user: { lock.user.clone() }
    };
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
