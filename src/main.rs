mod handlers;
mod models;
mod streaming;
mod video;

use std::{ collections::HashMap, sync::Arc };

use anyhow::{ Context, Result };
use axum::{ Router, routing::{ get, post } };
use tokio::{ fs, net::TcpListener, sync::RwLock };
use tower_http::services::ServeDir;

use models::{ AppConfig, AppState };
use streaming::start_tv_loop_if_needed;

/// Load the default configuration if it exists
async fn load_default_config(hls_root: &std::path::Path) -> AppConfig {
    let config_file = hls_root.join("configs").join("default.json");

    if config_file.exists() {
        match tokio::fs::read_to_string(&config_file).await {
            Ok(content) =>
                match serde_json::from_str::<AppConfig>(&content) {
                    Ok(config) => {
                        println!("Loaded default configuration from {}", config_file.display());
                        return config;
                    }
                    Err(e) => {
                        println!("Failed to parse default configuration: {}", e);
                    }
                }
            Err(e) => {
                println!("Failed to read default configuration: {}", e);
            }
        }
    }

    println!("No default configuration found, using empty configuration");
    AppConfig::default()
}

#[tokio::main]
async fn main() -> Result<()> {
    let hls_root = std::env::temp_dir().join("Rurushi-hls");
    fs::create_dir_all(&hls_root).await?;
    let config = load_default_config(&hls_root).await;

    let mut tv_files = Vec::new();
    if !config.shows.is_empty() {
        for episodes in config.shows.values() {
            for episode in episodes {
                tv_files.push(episode.file_path.clone());
            }
        }
    }

    let state = Arc::new(AppState {
        tv_files: RwLock::new(tv_files),
        hls_root,
        jobs: RwLock::new(HashMap::new()),
        videos_folder: RwLock::new(config.videos_folder),
        shows: RwLock::new(config.shows),
        playlist: RwLock::new(config.playlist),
        played_episodes: Arc::new(RwLock::new(config.played_episodes)),
        subtitle_mode: RwLock::new(config.subtitle_mode),
    });

    start_tv_loop_if_needed(state.clone()).await;

    let router = Router::new()
        .route("/webui", get(handlers::root))
        .route("/api/channels", get(handlers::list_channels))
        .route("/stream/{id}", get(handlers::stream_m3u8))
        .route("/api/videos-folder", get(handlers::get_videos_folder))
        .route("/api/videos-folder", post(handlers::set_videos_folder))
        .route("/api/scan-videos", post(handlers::scan_videos_folder))
        .route("/api/shows", get(handlers::get_shows))
        .route("/api/playlist", get(handlers::get_playlist))
        .route("/api/playlist", post(handlers::set_playlist))
        .route("/api/playlist/auto", post(handlers::enable_auto_mode))
        .route("/api/playlist/status", get(handlers::get_playlist_status))
        .route("/api/playlist/save", post(handlers::save_playlist))
        .route("/api/playlist/load", post(handlers::load_playlist))
        .route("/api/configs", get(handlers::list_configs))
        .route("/api/config/save", post(handlers::save_config))
        .route("/api/config/load", post(handlers::load_config))
        .route("/api/subtitle-mode", get(handlers::get_subtitle_mode))
        .route("/api/subtitle-mode", post(handlers::set_subtitle_mode))
        .route("/api/start-streaming", post(handlers::start_streaming))
        .nest_service("/hls", ServeDir::new(state.hls_root.clone()))
        .with_state(state.clone());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("HLS server listening on http://{addr}");
    println!("- Channel list: http://{addr}/api/channels");
    println!("- HLS files served under /hls");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router).await.context("server error")?;
    Ok(())
}
