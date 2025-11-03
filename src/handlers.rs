use std::{ sync::Arc, time::Duration };
use axum::{
    extract::{ Path as AxPath, State },
    http::{ StatusCode, Uri, HeaderMap },
    response::{ IntoResponse, Redirect, Response },
};

use crate::models::AppState;
use crate::streaming::{ start_tv_loop_if_needed, wait_for_file };

pub async fn stream_m3u8(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    _uri: Uri,
    _headers: HeaderMap
) -> Result<Response, (StatusCode, String)> {
    if id != "tv" {
        return Err((StatusCode::NOT_FOUND, format!("Unknown channel: {}", id)));
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

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
