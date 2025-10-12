use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use axum::{
    extract::{Path as AxPath, State},
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Redirect, Response},
    Json,
};

use crate::models::{
    AppState, ChannelInfo, FolderScanResult, LoadPlaylistRequest, SavePlaylistRequest,
    SetPlaylistRequest, SetVideosFolderRequest, AppConfig, SaveConfigRequest, LoadConfigRequest,
};
use crate::streaming::{start_tv_loop_if_needed, wait_for_file};
use crate::video::{organize_shows_and_episodes, scan_for_videos};

pub async fn root() -> impl IntoResponse {
    let body = r#"<html><body>
        <h1>Rurushi HLS</h1>
        <div id="folder-section">
            <h2>Videos Folder</h2>
            <div>
                <input type="text" id="folder-path" placeholder="Enter videos folder path" style="width: 400px;">
                <button onclick="setVideosFolder()">Set Folder</button>
                <button onclick="scanVideosFolder()">Scan for Videos</button>
            </div>
            <div id="folder-status" style="margin-top: 10px;"></div>
        </div>

        <div id="playlist-section" style="margin-top: 20px;">
            <h2>Program Scheduling</h2>
            <div>
                <button onclick="loadShows()">Load Shows</button>
                <button onclick="enableAutoMode()">Enable Auto Mode</button>
                <button onclick="loadPlaylistStatus()">Refresh Status</button>
            </div>
            <div id="playlist-status" style="margin-top: 10px;"></div>
            <div id="shows-section" style="margin-top: 10px; display: none;">
                <h3>Available Shows</h3>
                <div id="shows-list"></div>
            </div>
            <div id="playlist-section" style="margin-top: 10px;">
                <h3>Current Playlist</h3>
                <div id="playlist-list"></div>
            </div>
        </div>

        <div id="config-section" style="margin-top: 20px;">
            <h2>Configuration Management</h2>
            <div style="margin-bottom: 15px;">
                <input type="text" id="config-name" placeholder="Configuration name" style="width: 300px; margin-right: 10px;">
                <button onclick="saveConfig()" style="padding: 8px 16px; background: #28a745; color: white; border: none; border-radius: 3px; cursor: pointer; margin-right: 10px;">Save Config</button>
                <button onclick="loadConfig()" style="padding: 8px 16px; background: #17a2b8; color: white; border: none; border-radius: 3px; cursor: pointer; margin-right: 10px;">Load Config</button>
                <button onclick="loadConfigsList()" style="padding: 8px 16px; background: #6c757d; color: white; border: none; border-radius: 3px; cursor: pointer;">List Configs</button>
            </div>
            <div id="config-status" style="margin-top: 10px;"></div>
            <div id="configs-list" style="margin-top: 10px; display: none;">
                <h3>Saved Configurations</h3>
                <div id="configs-container"></div>
            </div>
        </div>

        <div id="channels-section" style="margin-top: 20px;">
            <h2>Channels</h2>
            <p>List channels: <a href="/api/channels">/api/channels</a></p>
            <p>Example play URL: /stream/tv</p>
            <div style="margin-bottom: 15px;">
                <button onclick="startStreaming()" id="start-streaming-btn" style="padding: 10px 20px; background: #007bff; color: white; border: none; border-radius: 5px; cursor: pointer;">Start Streaming</button>
                <span id="streaming-status" style="margin-left: 10px;"></span>
            </div>
            <div style="margin-bottom: 10px; padding: 8px; background: #e7f3ff; border-radius: 3px; font-size: 14px;">
                <strong>Setup Steps:</strong>
                1. Set your videos folder above → 2. Click "Scan for Videos" → 3. Click "Start Streaming"
            </div>
            <div id="channels-info"></div>
        </div>

        <script>
            async function setVideosFolder() {
                const path = document.getElementById('folder-path').value;
                if (!path) {
                    alert('Please enter a folder path');
                    return;
                }

                try {
                    const response = await fetch('/api/videos-folder', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                        },
                        body: JSON.stringify({ path: path })
                    });

                    const result = await response.json();
                    document.getElementById('folder-status').innerHTML =
                        `<p style="color: ${result.success ? 'green' : 'red'}">${result.message}</p>`;
                } catch (error) {
                    document.getElementById('folder-status').innerHTML =
                        `<p style="color: red">Error: ${error.message}</p>`;
                }
            }

            async function scanVideosFolder() {
                try {
                    const response = await fetch('/api/scan-videos', {
                        method: 'POST'
                    });

                    const result = await response.json();
                    document.getElementById('folder-status').innerHTML =
                        `<p style="color: ${result.success ? 'green' : 'red'}">${result.message}</p>`;

                    if (result.success) {
                        loadChannels();
                    }
                } catch (error) {
                    document.getElementById('folder-status').innerHTML =
                        `<p style="color: red">Error: ${error.message}</p>`;
                }
            }

            async function loadChannels() {
                try {
                    const response = await fetch('/api/channels');
                    const channels = await response.json();

                    const channelsDiv = document.getElementById('channels-info');
                    if (channels.length > 0) {
                        const channel = channels[0];
                        channelsDiv.innerHTML = `
                            <h3>TV Channel</h3>
                            <p><strong>Sources:</strong> ${channel.sources.length} files</p>
                            <p><strong>Stream URL:</strong> <a href="${channel.m3u8}">${channel.m3u8}</a></p>
                        `;
                    }
                } catch (error) {
                    console.error('Error loading channels:', error);
                }
            }

            async function loadShows() {
                try {
                    const response = await fetch('/api/shows');
                    const shows = await response.json();

                    const showsDiv = document.getElementById('shows-list');
                    let html = '<div style="margin-bottom: 20px;">';

                    for (const [showName, episodes] of Object.entries(shows)) {
                        html += `<div style="margin-bottom: 15px; padding: 10px; border: 1px solid #ccc; border-radius: 5px;">`;
                        html += `<h4>${showName} (${episodes.length} episodes)</h4>`;
                        html += `<div style="margin-left: 20px;">`;

                        episodes.forEach(ep => {
                            const epNum = ep.episode_number ? `Episode ${ep.episode_number}` : 'Unknown Episode';
                            html += `<div style="margin: 5px 0;">${epNum}: ${ep.name}</div>`;
                        });

                        html += `</div></div>`;
                    }

                    html += '</div>';
                    showsDiv.innerHTML = html;
                    document.getElementById('shows-section').style.display = 'block';
                } catch (error) {
                    console.error('Error loading shows:', error);
                }
            }

            async function enableAutoMode() {
                try {
                    const response = await fetch('/api/playlist/auto', {
                        method: 'POST'
                    });

                    const result = await response.json();
                    document.getElementById('playlist-status').innerHTML =
                        `<p style="color: ${result.success ? 'green' : 'red'}">${result.message}</p>`;

                    if (result.success) {
                        loadPlaylist();
                    }
                } catch (error) {
                    document.getElementById('playlist-status').innerHTML =
                        `<p style="color: red">Error: ${error.message}</p>`;
                }
            }

            async function loadPlaylist() {
                try {
                    const response = await fetch('/api/playlist');
                    const playlist = await response.json();

                    const playlistDiv = document.getElementById('playlist-list');
                    if (playlist.length === 0) {
                        playlistDiv.innerHTML = '<p>No playlist set. Use Auto Mode or create a custom playlist.</p>';
                        return;
                    }

                    let html = '';
                    playlist.forEach((item, index) => {
                        const rangeText = item.episode_range ?
                            `Episodes ${item.episode_range[0]}-${item.episode_range[1]}` :
                            'All Episodes';
                        const repeatText = item.repeat_count > 0 ?
                            `(Repeat ${item.repeat_count}x)` :
                            '(No Repeat)';

                        html += `<div style="margin: 5px 0; padding: 5px; background: #f0f0f0; border-radius: 3px;">
                            ${index + 1}. ${item.show_name} - ${rangeText} ${repeatText}
                        </div>`;
                    });

                    playlistDiv.innerHTML = html;
                } catch (error) {
                    console.error('Error loading playlist:', error);
                }
            }

            async function loadPlaylistStatus() {
                try {
                    const response = await fetch('/api/playlist/status');
                    const status = await response.json();

                    const statusDiv = document.getElementById('playlist-status');
                    statusDiv.innerHTML = `
                        <div style="padding: 10px; background: #f9f9f9; border-radius: 5px;">
                            <p><strong>Playlist Items:</strong> ${status.playlist_items}</p>
                            <p><strong>Total Episodes:</strong> ${status.total_episodes}</p>
                            <p><strong>Played Episodes:</strong> ${status.played_episodes}</p>
                            <p><strong>Progress:</strong> ${status.progress_percentage}%</p>
                            <p><strong>Mode:</strong> ${status.is_auto_mode ? 'Auto Mode' : 'Custom Playlist'}</p>
                        </div>
                    `;

                    if (status.playlist_items > 0) {
                        loadPlaylist();
                    }
                } catch (error) {
                    console.error('Error loading playlist status:', error);
                }
            }

            async function startStreaming() {
                const statusSpan = document.getElementById('streaming-status');
                const button = document.getElementById('start-streaming-btn');

                // Disable button and show loading state
                button.disabled = true;
                button.textContent = 'Starting...';
                statusSpan.textContent = '';

                try {
                    const response = await fetch('/api/start-streaming', {
                        method: 'POST'
                    });

                    const result = await response.json();

                    if (result.success) {
                        statusSpan.innerHTML = '<span style="color: green">Streaming active - Access stream at /stream/tv</span>';
                        button.textContent = 'Streaming Active';
                        button.style.background = '#28a745';

                        // Refresh channels info to show updated status
                        loadChannels();
                    } else {
                        statusSpan.innerHTML = `<span style="color: red">${result.message}</span>`;
                        button.textContent = 'Start Streaming';
                        button.disabled = false;
                        button.style.background = '#007bff';
                    }
                } catch (error) {
                    statusSpan.innerHTML = `<span style="color: red">Network error: ${error.message}</span>`;
                    button.textContent = 'Start Streaming';
                    button.disabled = false;
                    button.style.background = '#007bff';
                }
            }

            // Load channels on page load
            loadChannels();

            // Configuration management functions
            async function saveConfig() {
                const configName = document.getElementById('config-name').value.trim();
                if (!configName) {
                    alert('Please enter a configuration name');
                    return;
                }

                try {
                    const response = await fetch('/api/config/save', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                        },
                        body: JSON.stringify({ name: configName })
                    });

                    const result = await response.json();
                    document.getElementById('config-status').innerHTML =
                        `<p style="color: ${result.success ? 'green' : 'red'}">${result.message}</p>`;

                    if (result.success) {
                        document.getElementById('config-name').value = '';
                        loadConfigsList();
                    }
                } catch (error) {
                    document.getElementById('config-status').innerHTML =
                        `<p style="color: red">Error: ${error.message}</p>`;
                }
            }

            async function loadConfig() {
                const configName = document.getElementById('config-name').value.trim();
                if (!configName) {
                    alert('Please enter a configuration name');
                    return;
                }

                try {
                    const response = await fetch('/api/config/load', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                        },
                        body: JSON.stringify({ name: configName })
                    });

                    const result = await response.json();
                    document.getElementById('config-status').innerHTML =
                        `<p style="color: ${result.success ? 'green' : 'red'}">${result.message}</p>`;

                    if (result.success) {
                        // Refresh all data after loading config
                        loadChannels();
                        loadShows();
                        loadPlaylist();
                        loadPlaylistStatus();
                    }
                } catch (error) {
                    document.getElementById('config-status').innerHTML =
                        `<p style="color: red">Error: ${error.message}</p>`;
                }
            }

            async function loadConfigsList() {
                try {
                    const response = await fetch('/api/configs');
                    const configs = await response.json();

                    const container = document.getElementById('configs-container');
                    if (configs.length === 0) {
                        container.innerHTML = '<p>No saved configurations found.</p>';
                    } else {
                        let html = '<div style="display: flex; flex-wrap: wrap; gap: 10px;">';
                        configs.forEach(config => {
                            html += `
                                <div style="padding: 8px; background: #f8f9fa; border: 1px solid #dee2e6; border-radius: 3px;">
                                    <strong>${config}</strong>
                                    <br>
                                    <button onclick="loadConfigFromList('${config}')" style="margin-top: 5px; padding: 4px 8px; background: #17a2b8; color: white; border: none; border-radius: 3px; cursor: pointer; font-size: 12px;">Load</button>
                                </div>
                            `;
                        });
                        html += '</div>';
                        container.innerHTML = html;
                    }

                    document.getElementById('configs-list').style.display = 'block';
                } catch (error) {
                    console.error('Error loading configs list:', error);
                }
            }

            function loadConfigFromList(configName) {
                document.getElementById('config-name').value = configName;
                loadConfig();
            }
        </script>
    </body></html>"#;
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(body.to_string())
        .unwrap()
}

pub async fn list_channels(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let sources = state
        .tv_files
        .read()
        .await
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>();
    Json(vec![ChannelInfo {
        id: "tv".into(),
        sources,
        m3u8: "/stream/tv".into(),
    }])
}

pub async fn stream_m3u8(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    _uri: Uri,
    _headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    if id != "tv" {
        return Err((StatusCode::NOT_FOUND, format!("unknown channel: {id}")));
    }

    let channel_dir = state.hls_root.join("tv");
    let playlist = channel_dir.join("index.m3u8");

    if !playlist.exists() {
        println!("[stream] HLS playlist not found, starting TV loop...");
        start_tv_loop_if_needed(state.clone()).await;

        println!("[stream] Waiting for HLS playlist at: {}", playlist.display());
        let started = wait_for_file(&playlist, Duration::from_secs(8)).await;
        if !started {
            println!("[stream] TIMEOUT: HLS playlist did not appear at: {}", playlist.display());
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Timed out waiting for HLS playlist".into(),
            ));
        }
    }

    let redirect = "/hls/tv/index.m3u8".to_string();
    Ok(Redirect::temporary(&redirect).into_response())
}

pub async fn get_videos_folder(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let folder = state.videos_folder.read().await;
    Json(serde_json::json!({
        "videos_folder": folder.as_ref().map(|p| p.display().to_string())
    }))
}

pub async fn set_videos_folder(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SetVideosFolderRequest>,
) -> impl IntoResponse {
    let path = PathBuf::from(request.path);
    if !path.exists() || !path.is_dir() {
        return Json(FolderScanResult {
            success: false,
            video_count: 0,
            videos_folder: None,
            message: "Invalid folder path".to_string(),
        });
    }

    *state.videos_folder.write().await = Some(path.clone());
    Json(FolderScanResult {
        success: true,
        video_count: 0,
        videos_folder: Some(path.display().to_string()),
        message: "Videos folder set successfully".to_string(),
    })
}

pub async fn scan_videos_folder(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    println!("[scan-api] Starting video folder scan...");
    let folder = state.videos_folder.read().await.clone();
    if folder.is_none() {
        println!("[scan-api] No videos folder set");
        return Json(FolderScanResult {
            success: false,
            video_count: 0,
            videos_folder: None,
            message: "No videos folder set".to_string(),
        });
    }

    let folder = folder.unwrap();
    println!("[scan-api] Scanning folder: {}", folder.display());
    let video_files = scan_for_videos(&folder).await;
    println!("[scan-api] Found {} video files", video_files.len());

    let shows = organize_shows_and_episodes(&video_files).await;
    println!("[scan-api] Organized into {} shows", shows.len());

    *state.tv_files.write().await = video_files.clone();
    *state.shows.write().await = shows.clone();

    println!(
        "Found {} video files across {} shows",
        video_files.len(),
        shows.len()
    );

    Json(FolderScanResult {
        success: true,
        video_count: video_files.len(),
        videos_folder: Some(folder.display().to_string()),
        message: format!(
            "Found {} video files across {} shows",
            video_files.len(),
            shows.len()
        ),
    })
}

pub async fn get_shows(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let shows = state.shows.read().await.clone();
    let mut result = HashMap::new();

    for (show_name, episodes) in shows {
        let episode_info: Vec<serde_json::Value> = episodes
            .into_iter()
            .map(|ep| {
                serde_json::json!({
                    "id": ep.id,
                    "name": ep.name,
                    "episode_number": ep.episode_number,
                    "file_path": ep.file_path.display().to_string()
                })
            })
            .collect();

        result.insert(show_name, episode_info);
    }

    Json(result)
}

pub async fn get_playlist(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let playlist = state.playlist.read().await.clone();
    Json(playlist)
}

pub async fn set_playlist(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SetPlaylistRequest>,
) -> impl IntoResponse {
    *state.playlist.write().await = request.playlist.clone();
    *state.played_episodes.write().await = HashMap::new();

    Json(serde_json::json!({
        "success": true,
        "message": format!("Playlist set with {} items", request.playlist.len())
    }))
}

pub async fn enable_auto_mode(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let shows = state.shows.read().await.clone();

    if shows.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "message": "No shows available for auto mode"
        }));
    }

    let mut playlist = Vec::new();
    for show_name in shows.keys() {
        playlist.push(crate::models::PlaylistItem {
            show_name: show_name.clone(),
            episode_range: None,
            repeat_count: 0,
        });
    }

    *state.playlist.write().await = playlist.clone();
    *state.played_episodes.write().await = HashMap::new();

    Json(serde_json::json!({
        "success": true,
        "message": format!("Auto mode enabled with {} shows", playlist.len())
    }))
}

pub async fn get_playlist_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let playlist = state.playlist.read().await.clone();
    let played_episodes = state.played_episodes.read().await.clone();
    let shows = state.shows.read().await.clone();

    let mut total_episodes = 0;
    let mut played_total = 0;
    let mut shows_count = 0;

    for (show_name, episodes) in &shows {
        shows_count += 1;
        total_episodes += episodes.len();
        if let Some(played) = played_episodes.get(show_name) {
            played_total += played.len();
        }
    }

    Json(serde_json::json!({
        "playlist_items": playlist.len(),
        "total_episodes": total_episodes,
        "played_episodes": played_total,
        "progress_percentage": if total_episodes > 0 {
            (played_total as f32 / total_episodes as f32 * 100.0) as usize
        } else {
            0
        },
        "is_auto_mode": playlist.len() == shows_count && shows.values().all(|eps| eps.len() > 0)
    }))
}

pub async fn save_playlist(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SavePlaylistRequest>,
) -> impl IntoResponse {
    let playlist = state.playlist.read().await.clone();

    if playlist.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "message": "No playlist to save"
        }));
    }

    let playlist_file = state
        .hls_root
        .join("playlists")
        .join(format!("{}.json", request.name));

    if let Err(e) = tokio::fs::create_dir_all(playlist_file.parent().unwrap()).await {
        return Json(serde_json::json!({
            "success": false,
            "message": format!("Failed to create playlists directory: {}", e)
        }));
    }

    match tokio::fs::write(
        &playlist_file,
        serde_json::to_string_pretty(&playlist).unwrap(),
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "message": format!("Playlist saved as {}", request.name)
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "message": format!("Failed to save playlist: {}", e)
        })),
    }
}

/// Load a saved playlist from a file
pub async fn load_playlist(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoadPlaylistRequest>,
) -> impl IntoResponse {
    let playlist_file = state
        .hls_root
        .join("playlists")
        .join(format!("{}.json", request.name));

    match tokio::fs::read_to_string(&playlist_file).await {
        Ok(content) => {
            match serde_json::from_str::<Vec<crate::models::PlaylistItem>>(&content) {
                Ok(playlist) => {
                    *state.playlist.write().await = playlist.clone();
                    *state.played_episodes.write().await = HashMap::new(); // Reset tracking

                    Json(serde_json::json!({
                        "success": true,
                        "message": format!("Playlist '{}' loaded with {} items", request.name, playlist.len())
                    }))
                }
                Err(e) => Json(serde_json::json!({
                    "success": false,
                    "message": format!("Invalid playlist format: {}", e)
                })),
            }
        }
        Err(_) => Json(serde_json::json!({
            "success": false,
            "message": format!("Playlist '{}' not found", request.name)
        })),
    }
}

pub async fn save_config(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SaveConfigRequest>,
) -> impl IntoResponse {
    let videos_folder = state.videos_folder.read().await.clone();
    let shows = state.shows.read().await.clone();
    let playlist = state.playlist.read().await.clone();
    let played_episodes = state.played_episodes.read().await.clone();

    let config = AppConfig {
        videos_folder,
        shows,
        playlist,
        played_episodes,
    };

    let config_file = state
        .hls_root
        .join("configs")
        .join(format!("{}.json", request.name));

    if let Err(e) = tokio::fs::create_dir_all(config_file.parent().unwrap()).await {
        return Json(serde_json::json!({
            "success": false,
            "message": format!("Failed to create configs directory: {}", e)
        }));
    }

    match tokio::fs::write(
        &config_file,
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "message": format!("Configuration saved as {}", request.name)
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "message": format!("Failed to save configuration: {}", e)
        })),
    }
}

pub async fn load_config(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoadConfigRequest>,
) -> impl IntoResponse {
    let config_file = state
        .hls_root
        .join("configs")
        .join(format!("{}.json", request.name));

    match tokio::fs::read_to_string(&config_file).await {
        Ok(content) => {
            match serde_json::from_str::<AppConfig>(&content) {
                Ok(config) => {
                    *state.videos_folder.write().await = config.videos_folder.clone();
                    *state.shows.write().await = config.shows.clone();
                    *state.playlist.write().await = config.playlist.clone();
                    *state.played_episodes.write().await = config.played_episodes.clone();

                    if !config.shows.is_empty() {
                        let mut tv_files = Vec::new();
                        for episodes in config.shows.values() {
                            for episode in episodes {
                                tv_files.push(episode.file_path.clone());
                            }
                        }
                        *state.tv_files.write().await = tv_files;
                    }

                    Json(serde_json::json!({
                        "success": true,
                        "message": format!("Configuration '{}' loaded successfully", request.name)
                    }))
                }
                Err(e) => Json(serde_json::json!({
                    "success": false,
                    "message": format!("Invalid configuration format: {}", e)
                })),
            }
        }
        Err(_) => Json(serde_json::json!({
            "success": false,
            "message": format!("Configuration '{}' not found", request.name)
        })),
    }
}

pub async fn list_configs(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let configs_dir = state.hls_root.join("configs");

    if let Err(_) = tokio::fs::create_dir_all(&configs_dir).await {
        return Json(Vec::<String>::new());
    }

    match tokio::fs::read_dir(&configs_dir).await {
        Ok(mut entries) => {
            let mut configs = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") {
                        if let Some(config_name) = name.strip_suffix(".json") {
                            configs.push(config_name.to_string());
                        }
                    }
                }
            }
            Json(configs)
        }
        Err(_) => Json(Vec::<String>::new()),
    }
}

pub async fn start_streaming(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tv_files = state.tv_files.read().await;
    let shows = state.shows.read().await;

    println!("[api] Start streaming requested - tv_files: {}, shows: {}", tv_files.len(), shows.len());

    if tv_files.is_empty() && shows.is_empty() {
        println!("[api] No video files or shows available for streaming");
        return Json(serde_json::json!({
            "success": false,
            "message": "No video files available. Please set a videos folder and scan for videos first."
        }));
    }

    let playlist = state.playlist.read().await;
    if playlist.is_empty() && tv_files.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "message": "No playlist set and no video files available. Please scan for videos or create a playlist."
        }));
    }

    println!("[api] Starting TV loop...");
    start_tv_loop_if_needed(state.clone()).await;
    println!("[api] TV loop started successfully");

    Json(serde_json::json!({
        "success": true,
        "message": "Streaming started successfully"
    }))
}
