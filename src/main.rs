mod db;
mod pages;
mod scraper;

use std::sync::Arc;
use tokio::sync::Mutex;

use anyhow::{self, Context};
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use pages::{
    anime::seasonal_anime,
    home::{
        clear_transmission, close, confirm_match, create_filter, create_show_filter,
        currently_airing_anime, delete_filter, delete_show_filter, download_from_link,
        get_configuration, get_filters, get_rss_config, get_show_filters, get_source,
        navigate_season_bar, navigate_seasonal_anime, save_configuration, save_rss_config,
        search_matches, search_source,
        set_tracker, show_table, skip_match_selection, sync_now, toggle_filter, update_filter,
        update_user, view, UserState,
    },
};
use scraper::tracker::run_tracker;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::pages::home::read_tracked_shows;

#[tokio::main(worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "axum_static_web_werver=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("initializing router and assets");

    // Initialize the database connection and run migrations
    db::init_connection().context("Failed to initialize database connection")?;
    db::with_db(|conn| {
        db::init_database(conn)?;
        db::migrate_from_json_if_needed(conn)
    })
    .await
    .context("Failed to initialize database schema or migrate data")?;

    info!("database initialized successfully");

    // Use port env if available
    let port = std::env::var("PORT").unwrap_or_else(|_| "42069".to_string());
    let port = port
        .parse()
        .context("PORT environment variable is not a vlid u16")?;

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    eprintln!("Listening on http://{}", addr);
    info!("router initalized, now listening on port {}", port);

    let state = AppState {
        user: Arc::new(Mutex::new(UserState::new(
            "Yeehaw".to_string(),
            read_tracked_shows().await?,
        ))),
    };

    tokio::spawn(run_tracker());

    let listener = TcpListener::bind(&addr).await.context("failed to bind TCP listener")?;
    axum::serve(listener, router(state)?)
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
        .route("/download_from_link", post(download_from_link))
        .route("/search_source", post(search_source))
        .route("/save_configuration", post(save_configuration))
        .route(
            "/show_table",
            get(show_table).with_state(state.user.clone()),
        )
        .route(
            "/navigate_seasonal_anime",
            get(navigate_seasonal_anime).with_state(state.user.clone()),
        )
        .route("/navigate_season_bar", get(navigate_season_bar))
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
        .route(
            "/rss_config",
            get(get_rss_config).post(save_rss_config),
        )
        .route("/anime", get(seasonal_anime))
        .route("/close", get(close))
        .route("/sync_now", post(sync_now))
        .route("/clear_transmission", post(clear_transmission))
        .route("/search_matches", get(search_matches))
        .route(
            "/confirm_match",
            post(confirm_match).with_state(state.user.clone()),
        )
        .route(
            "/skip_match_selection",
            post(skip_match_selection).with_state(state.user.clone()),
        )
        // Filter management routes
        .route("/filters", get(get_filters).post(create_filter))
        .route(
            "/filters/:id",
            put(update_filter).delete(delete_filter),
        )
        .route("/filters/:id/toggle", post(toggle_filter))
        // Show-specific filter routes
        .route(
            "/shows/:show_id/filters",
            get(get_show_filters).post(create_show_filter),
        )
        .route(
            "/shows/:show_id/filters/:filter_id",
            delete(delete_show_filter),
        )
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

// #[cfg(test)]
// mod test {
//     use super::*;
//     use crate::scraper::subsplease::get_magnet_links_from_subsplease;
//     use crate::scraper::transmission::upload_to_transmission_rpc;
//
//     #[tokio::test]
//     async fn test_scrape_subs_and_upload_batch_true() {
//         let sp_title = "Sousou no Frieren".to_string();
//         let season_number = Some(1);
//         let batch = true;
//         let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch, 4444).await;
//         assert!(magnet_links.is_ok());
//         let result =
//             upload_to_transmission_rpc(magnet_links.unwrap(), &sp_title, season_number).await;
//         assert!(result.is_ok());
//     }
//
//     #[tokio::test]
//     async fn test_scrape_subs_and_upload_batch_false() {
//         let sp_title = "Sousou no Frieren".to_string();
//         let season_number = Some(1);
//         let batch = false;
//         let magnet_links = get_magnet_links_from_subsplease(&sp_title, batch, 4445).await;
//         assert!(magnet_links.is_ok());
//         let result =
//             upload_to_transmission_rpc(magnet_links.unwrap(), &sp_title, season_number).await;
//         assert!(result.is_ok());
//     }
// }
