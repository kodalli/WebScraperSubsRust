mod pages;
mod scraper;

use std::sync::Arc;
use tokio::sync::Mutex;

use anyhow::{self, Context};
use axum::{
    routing::{get, post},
    Router,
};
use pages::{
    anime::seasonal_anime,
    home::{
        currently_airing_anime, get_source, navigate_seasonal_anime, set_tracker, show_table,
        update_user, view, UserState, download_from_link, search_source, get_configuration, save_configuration, close,
    },
};
use scraper::subsplease::get_magnet_links_from_subsplease;
use scraper::transmission::upload_to_transmission_rpc;
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::pages::home::read_tracked_shows;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "axum_static_web_werver=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("initializing router and assets");

    // Use port env if available
    let port = std::env::var("PORT").unwrap_or_else(|_| "42069".to_string());
    let port = port
        .parse()
        .context("PORT environment variable is not a vlid u16")?;

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));

    eprintln!("Listening on http://{}", addr);
    info!("router initalized, now listening on port {}", port);

    let state = AppState {
        user: Arc::new(Mutex::new(UserState::new(
            "Yeehaw".to_string(),
            read_tracked_shows().await?,
        ))),
    };

    axum::Server::bind(&addr)
        .serve(router(state)?.into_make_service())
        .await
        .context("error while starting server")?;

    Ok(())
}

struct AppState {
    user: Arc<Mutex<UserState>>,
}

fn api_router(state: AppState) -> Router {
    // clone on arc just increases reference count
    Router::new()
        .route("/login", post(update_user).with_state(state.user.clone()))
        .route(
            "/set_tracker",
            post(set_tracker).with_state(state.user.clone()),
        )
        .route(
            "/download_from_link",
            post(download_from_link),
        )
        .route(
            "/search_source",
            post(search_source),
        )
        .route(
            "/save_configuration",
            post(save_configuration),
        )
        .route(
            "/show_table",
            get(show_table).with_state(state.user.clone()),
        )
        .route(
            "/navigate_seasonal_anime",
            get(navigate_seasonal_anime).with_state(state.user.clone()),
        )
        .route(
            "/currently_airing",
            get(currently_airing_anime).with_state(state.user.clone()),
        )
        .route(
            "/get_source",
            get(get_source).with_state(state.user.clone()),
        )
        .route(
            "/get_configuration",
            get(get_configuration).with_state(state.user.clone()),
        )
        .route("/anime", get(seasonal_anime))
        .route("/close", get(close))
}

fn router(state: AppState) -> anyhow::Result<Router> {
    let assets_path = std::env::current_dir()?;
    let assets_serve_dir = ServeDir::new(format!(
        "{}/assets",
        assets_path.to_str().expect("assets path is not valid utf8")
    ));

    Ok(Router::new()
        .route("/", get(view).with_state(state.user.clone()))
        .nest(
            "/api",
            api_router(AppState {
                user: state.user.clone(),
            }),
        )
        .nest_service("/assets", assets_serve_dir))
}

//#[tokio::main]
//async fn main() -> Result<()> {
//    // Prompt user input
//    let (sp_title, season_number, batch) = get_user_input();
//    // This needs a running web driver like chromedriver or geckodriver
//    let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch, 4444).await?;
//    let _result = upload_to_transmission_rpc(magnet_links, &sp_title, season_number).await?;
//
//    Ok(())
//}
//
//fn get_user_input() -> (String, u8, bool) {
//    let mut sp_title = String::new();
//    println!("Enter the subsplease title: ");
//    std::io::stdin()
//        .read_line(&mut sp_title)
//        .expect("Could not read arg");
//    sp_title = sp_title.replace("–", "-").trim().to_string();
//
//    let mut season_str = String::new();
//    println!("Enter the season: ");
//    std::io::stdin()
//        .read_line(&mut season_str)
//        .expect("Could not read arg");
//    let season_number = season_str.trim().parse::<u8>().unwrap();
//
//    let mut batch_str = String::new();
//    println!("Enter true or false for batch download: ");
//    std::io::stdin()
//        .read_line(&mut batch_str)
//        .expect("Could not read arg");
//    let batch = batch_str.trim().parse::<bool>().unwrap();
//
//    (sp_title, season_number, batch)
//}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_scrape_subs_and_upload_batch_true() {
        let sp_title = "Sousou no Frieren".to_string();
        let season_number = Some(1);
        let batch = true;
        let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch, 4444).await;
        assert!(magnet_links.is_ok());
        let result =
            upload_to_transmission_rpc(magnet_links.unwrap(), &sp_title, season_number).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_scrape_subs_and_upload_batch_false() {
        let sp_title = "Sousou no Frieren".to_string();
        let season_number = Some(1);
        let batch = false;
        let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch, 4445).await;
        assert!(magnet_links.is_ok());
        let result =
            upload_to_transmission_rpc(magnet_links.unwrap(), &sp_title, season_number).await;
        assert!(result.is_ok());
    }
}
