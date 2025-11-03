mod api;
mod handlers;
mod models;
mod streaming;
mod video;

use std::{ collections::HashMap, sync::Arc };

use anyhow::{ Context, Result };
use axum::{ routing::{ delete, get, post }, Router };
use tokio::{ fs, sync::RwLock };
use tower_http::{ cors::CorsLayer, services::ServeDir };

use models::{ AppConfig, AppState };

async fn load_config() -> Result<AppConfig> {
    let exe_dir = std::env::current_exe()
        .context("Failed to get executable path")?
        .parent()
        .context("Failed to get executable directory")?
        .to_path_buf();
    
    let config_path = exe_dir.join("config.yml");
    
    if !config_path.exists() {
        println!("No config.yml found at {}, using default configuration", config_path.display());
        return Ok(AppConfig::default());
    }

    println!("Loading configuration from {}", config_path.display());
    let content = tokio::fs::read_to_string(&config_path).await
        .context("Failed to read config.yml")?;
    
    let config: AppConfig = serde_yaml::from_str(&content)
        .context("Failed to parse config.yml")?;
    
    println!("Configuration loaded successfully");
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let hls_root = std::env::temp_dir().join("Rurushi-hls");
    fs::create_dir_all(&hls_root).await?;
    let config = load_config().await?;

    let mut tv_files = Vec::new();
    if !config.shows.is_empty() {
        for episodes in config.shows.values() {
            for episode in episodes {
                tv_files.push(episode.file_path.clone());
            }
        }
        println!("Loaded {} video files from config", tv_files.len());
    }

    let state = Arc::new(AppState {
        tv_files: RwLock::new(tv_files.clone()),
        hls_root: hls_root.clone(),
        jobs: RwLock::new(HashMap::new()),
        videos_folder: RwLock::new(config.videos_folder.clone()),
        shows: RwLock::new(config.shows.clone()),
        playlist: RwLock::new(config.playlist.clone()),
        played_episodes: Arc::new(RwLock::new(config.played_episodes.clone())),
        subtitle_mode: RwLock::new(config.subtitle_mode.clone()),
        current_playing: RwLock::new(None),
        is_playing: RwLock::new(false),
    });

    println!("Starting Rurushi HLS Server with Axum API + Next.js WebUI...");
    println!("HLS output directory: {}", hls_root.display());

    let state_clone = state.clone();
    tokio::spawn(async move {
        streaming::stop_streaming(state_clone).await;
    });

    start_http_server(state, hls_root).await?;

    Ok(())
}

async fn start_http_server(state: Arc<AppState>, hls_root: std::path::PathBuf) -> Result<()> {
    let cors = CorsLayer::permissive();

    // Determine WebUI path (either built static files or dev server)
    let exe_dir = std::env::current_exe()
        .context("Failed to get executable path")?
        .parent()
        .context("Failed to get executable directory")?
        .to_path_buf();
    
    let webui_path = exe_dir.join("webui").join("out");
    let has_static_webui = webui_path.exists();

    let mut app = Router::new()
        // streaming endpoints
        .route("/stream/{id}", get(handlers::stream_m3u8))
        .route("/health", get(handlers::health_check))
        .nest_service("/hls", ServeDir::new(hls_root))
        // API endpoints
        .route("/api/config", get(api::get_config))
        .route("/api/folder", post(api::set_folder))
        .route("/api/scan", post(api::scan_videos))
        .route("/api/files", get(api::get_files))
        .route("/api/shows", get(api::get_shows))
        .route("/api/play", post(api::play_video))
        .route("/api/stop", post(api::stop_playback))
        .route("/api/start-streaming", post(api::start_streaming))
        .route("/api/subtitle-mode", post(api::set_subtitle_mode))
        .route("/api/playlist", get(api::get_playlist))
        .route("/api/playlist/add", post(api::add_to_playlist))
        .route("/api/playlist/{index}", delete(api::remove_from_playlist))
        .route("/api/playlist/move", post(api::move_playlist_item))
        .route("/api/playlist", delete(api::clear_playlist))
        .layer(cors)
        .with_state(state);

    // Serve static webui if available
    if has_static_webui {
        println!("[http] Serving static WebUI from: {}", webui_path.display());
        app = app.nest_service("/", ServeDir::new(webui_path));
    }

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("[http] Server listening on http://{}", addr);
    println!("[http] API available at http://{}/api", addr);
    println!("[http] Stream available at http://{}/stream/tv", addr);
    
    if has_static_webui {
        println!("[http] WebUI available at http://{}/", addr);
    } else {
        println!("[http] WebUI not found. For development, run: cd webui && npm run dev");
        println!("[http] For production, build WebUI first: cd webui && npm run build");
    }

    axum::serve(listener, app)
        .await
        .context("HTTP server error")?;

    Ok(())
}
