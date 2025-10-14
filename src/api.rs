use std::{ collections::HashMap, path::PathBuf, sync::Arc };
use axum::{
    extract::{ Path as AxPath, State },
    http::StatusCode,
    response::{ IntoResponse, Json },
};
use serde::{ Deserialize, Serialize };

use crate::models::{ AppConfig, AppState, Episode, PlaylistItem, SubtitleMode };
use crate::streaming::{ play_file, start_tv_loop_if_needed, stop_streaming };
use crate::video::{ organize_shows_and_episodes, scan_for_videos };

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

#[derive(Serialize)]
pub struct ConfigResponse {
    pub videos_folder: Option<String>,
    pub video_count: usize,
    pub show_count: usize,
    pub shows: HashMap<String, Vec<Episode>>,
    pub playlist: Vec<PlaylistItem>,
    pub subtitle_mode: SubtitleMode,
    pub is_streaming: bool,
    pub current_playing: Option<String>,
}

#[derive(Serialize)]
pub struct ScanResponse {
    pub video_count: usize,
    pub show_count: usize,
    pub shows: HashMap<String, Vec<Episode>>,
}

#[derive(Serialize)]
pub struct FileListResponse {
    pub files: Vec<FileInfo>,
}

#[derive(Serialize)]
pub struct FileInfo {
    pub display_name: String,
    pub file_path: String,
    pub show_name: String,
}

#[derive(Serialize)]
pub struct ShowListResponse {
    pub shows: Vec<String>,
}

#[derive(Deserialize)]
pub struct SetFolderRequest {
    pub path: String,
}

#[derive(Deserialize)]
pub struct PlayFileRequest {
    pub file_path: String,
}

#[derive(Deserialize)]
pub struct SetSubtitleModeRequest {
    pub mode: SubtitleMode,
}

#[derive(Deserialize)]
pub struct AddToPlaylistRequest {
    pub show_name: String,
    pub episode_range: Option<(usize, usize)>,
    pub repeat_count: Option<usize>,
}

#[derive(Deserialize)]
pub struct MovePlaylistItemRequest {
    pub index: usize,
    pub direction: String,
}

/// GET /api/config
pub async fn get_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let videos_folder = state.videos_folder
        .read().await
        .as_ref()
        .map(|p| p.display().to_string());
    let shows = state.shows.read().await.clone();
    let playlist = state.playlist.read().await.clone();
    let subtitle_mode = state.subtitle_mode.read().await.clone();
    let is_streaming = *state.is_playing.read().await;
    let current_playing = state.current_playing
        .read().await
        .as_ref()
        .map(|p| p.display().to_string());

    let video_count = shows
        .values()
        .map(|eps| eps.len())
        .sum();
    let show_count = shows.len();

    let response = ConfigResponse {
        videos_folder,
        video_count,
        show_count,
        shows,
        playlist,
        subtitle_mode,
        is_streaming,
        current_playing,
    };

    Json(ApiResponse::success(response))
}

/// POST /api/folder
pub async fn set_folder(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetFolderRequest>
) -> impl IntoResponse {
    let path = PathBuf::from(&req.path);

    if !path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("Folder does not exist".to_string())),
        );
    }

    *state.videos_folder.write().await = Some(path.clone());

    if let Err(e) = save_config_internal(state).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!("Failed to save config: {}", e))),
        );
    }

    (StatusCode::OK, Json(ApiResponse::success(())))
}

/// POST /api/scan
pub async fn scan_videos(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let videos_folder = state.videos_folder.read().await.clone();

    if videos_folder.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<ScanResponse>::error("No videos folder set".to_string())),
        );
    }

    let folder = videos_folder.unwrap();
    let video_files = scan_for_videos(&folder).await;
    let video_count = video_files.len();

    let shows = organize_shows_and_episodes(&video_files).await;
    let show_count = shows.len();

    *state.shows.write().await = shows.clone();

    let mut tv_files = Vec::new();
    for episodes in shows.values() {
        for episode in episodes {
            tv_files.push(episode.file_path.clone());
        }
    }
    *state.tv_files.write().await = tv_files;

    if let Err(e) = save_config_internal(state).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<ScanResponse>::error(format!("Failed to save config: {}", e))),
        );
    }

    let response = ScanResponse {
        video_count,
        show_count,
        shows,
    };

    (StatusCode::OK, Json(ApiResponse::success(response)))
}

/// GET /api/files
pub async fn get_files(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let shows = state.shows.read().await.clone();
    let mut files = Vec::new();

    for (show_name, episodes) in &shows {
        for episode in episodes {
            files.push(FileInfo {
                display_name: format!("{} - {}", show_name, episode.name),
                file_path: episode.file_path.display().to_string(),
                show_name: show_name.clone(),
            });
        }
    }

    let response = FileListResponse { files };
    Json(ApiResponse::success(response))
}

/// GET /api/shows
pub async fn get_shows(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let shows = state.shows.read().await;
    let mut show_names: Vec<String> = shows.keys().cloned().collect();
    show_names.sort();

    let response = ShowListResponse { shows: show_names };
    Json(ApiResponse::success(response))
}

/// POST /api/play
pub async fn play_video(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PlayFileRequest>
) -> impl IntoResponse {
    let file_path = PathBuf::from(&req.file_path);

    if !file_path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("File does not exist".to_string())),
        );
    }

    if let Err(e) = play_file(state, file_path).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!("Failed to play file: {}", e))),
        );
    }

    (StatusCode::OK, Json(ApiResponse::success(())))
}

/// POST /api/stop
pub async fn stop_playback(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    stop_streaming(state).await;
    Json(ApiResponse::success(()))
}

/// POST /api/start-streaming
pub async fn start_streaming(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let shows = state.shows.read().await;
    if shows.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("No videos available. Please scan first.".to_string())),
        );
    }
    drop(shows);

    start_tv_loop_if_needed(state).await;
    (StatusCode::OK, Json(ApiResponse::success(())))
}

/// POST /api/subtitle-mode
pub async fn set_subtitle_mode(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetSubtitleModeRequest>
) -> impl IntoResponse {
    *state.subtitle_mode.write().await = req.mode.clone();

    // Save config
    if let Err(e) = save_config_internal(state).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!("Failed to save config: {}", e))),
        );
    }

    (StatusCode::OK, Json(ApiResponse::success(())))
}

// Playlist Management Handlers

/// GET /api/playlist
pub async fn get_playlist(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let playlist = state.playlist.read().await.clone();
    Json(ApiResponse::success(playlist))
}

/// POST /api/playlist/add
pub async fn add_to_playlist(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddToPlaylistRequest>
) -> impl IntoResponse {
    let shows = state.shows.read().await;
    if !shows.contains_key(&req.show_name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("Show not found".to_string())),
        );
    }
    drop(shows);

    let item = PlaylistItem {
        show_name: req.show_name,
        episode_range: req.episode_range,
        repeat_count: req.repeat_count.unwrap_or(0),
    };

    state.playlist.write().await.push(item);

    if let Err(e) = save_config_internal(state).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!("Failed to save config: {}", e))),
        );
    }

    (StatusCode::OK, Json(ApiResponse::success(())))
}

/// DELETE /api/playlist/{index}
pub async fn remove_from_playlist(
    State(state): State<Arc<AppState>>,
    AxPath(index): AxPath<usize>
) -> impl IntoResponse {
    let mut playlist = state.playlist.write().await;

    if index >= playlist.len() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("Invalid playlist index".to_string())),
        );
    }

    playlist.remove(index);
    drop(playlist);

    if let Err(e) = save_config_internal(state).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!("Failed to save config: {}", e))),
        );
    }

    (StatusCode::OK, Json(ApiResponse::success(())))
}

/// POST /api/playlist/move
pub async fn move_playlist_item(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MovePlaylistItemRequest>
) -> impl IntoResponse {
    let mut playlist = state.playlist.write().await;

    if req.index >= playlist.len() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("Invalid playlist index".to_string())),
        );
    }

    match req.direction.as_str() {
        "up" => {
            if req.index == 0 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<()>::error("Cannot move first item up".to_string())),
                );
            }
            playlist.swap(req.index - 1, req.index);
        }
        "down" => {
            if req.index >= playlist.len() - 1 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<()>::error("Cannot move last item down".to_string())),
                );
            }
            playlist.swap(req.index, req.index + 1);
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error("Invalid direction. Use 'up' or 'down'".to_string())),
            );
        }
    }

    drop(playlist);

    if let Err(e) = save_config_internal(state).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!("Failed to save config: {}", e))),
        );
    }

    (StatusCode::OK, Json(ApiResponse::success(())))
}

/// DELETE /api/playlist
pub async fn clear_playlist(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.playlist.write().await.clear();

    // Save config
    if let Err(e) = save_config_internal(state).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!("Failed to save config: {}", e))),
        );
    }

    (StatusCode::OK, Json(ApiResponse::success(())))
}

// Helper Functions

async fn save_config_internal(state: Arc<AppState>) -> Result<(), String> {
    let exe_dir = std::env
        ::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?
        .parent()
        .ok_or_else(|| "Failed to get executable directory".to_string())?
        .to_path_buf();

    let config_path = exe_dir.join("config.yml");

    let config = AppConfig {
        videos_folder: state.videos_folder.read().await.clone(),
        shows: state.shows.read().await.clone(),
        playlist: state.playlist.read().await.clone(),
        played_episodes: state.played_episodes.read().await.clone(),
        subtitle_mode: state.subtitle_mode.read().await.clone(),
    };

    let yaml = serde_yaml
        ::to_string(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    tokio::fs
        ::write(&config_path, yaml).await
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    println!("Configuration saved to {}", config_path.display());
    Ok(())
}
