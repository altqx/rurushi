use std::{
    path::Path,
    process::Stdio,
    sync::{Arc, OnceLock},
    time::Duration,
};

use tokio::{fs, process::Command, time};

use crate::models::{AppState, SubtitleMode};

static FFMPEG_AVAILABLE: OnceLock<bool> = OnceLock::new();

async fn detect_subtitle_format(input_path: &Path) -> Result<bool, String> {
    let output = Command::new("ffprobe")
        .args(["-v", "quiet"])
        .args(["-print_format", "json"])
        .args(["-show_streams"])
        .args(["-select_streams", "s"])
        .arg(input_path.as_os_str())
        .output()
        .await
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        return Err("ffprobe failed to detect subtitle streams".to_string());
    }

    let json_output = String::from_utf8_lossy(&output.stdout);

    let text_codecs = ["subrip", "srt", "ass", "ssa", "mov_text", "webvtt", "text"];
    let bitmap_codecs = ["hdmv_pgs_subtitle", "dvd_subtitle", "dvdsub", "pgssub"];

    for codec in &text_codecs {
        if json_output.contains(codec) {
            println!("[subtitle] Detected text-based subtitle format: {}", codec);
            return Ok(true); // convert
        }
    }

    for codec in &bitmap_codecs {
        if json_output.contains(codec) {
            println!(
                "[subtitle] Detected bitmap-based subtitle format: {}",
                codec
            );
            return Ok(false); // burn
        }
    }

    if json_output.contains("\"codec_type\": \"subtitle\"") {
        println!("[subtitle] Found subtitles but format unknown, defaulting to burn");
        return Ok(false);
    }

    println!("[subtitle] No subtitle streams detected");
    Ok(true)
}

async fn build_ffmpeg_command(
    input_path: &Path,
    output_dir: &Path,
    subtitle_mode: &SubtitleMode,
) -> Command {
    let mut cmd = Command::new("ffmpeg");
    let seg_tmpl = output_dir.join("%09d.ts");

    cmd.arg("-re").arg("-i").arg(input_path.as_os_str());

    match subtitle_mode {
        SubtitleMode::None => {
            cmd.args(["-map", "0:v:0"])         
                .args(["-map", "0:a?"])
                .args(["-vf", "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2"]);
        }
        SubtitleMode::Smart => {
            let can_convert = detect_subtitle_format(input_path).await.unwrap_or(false);

            if can_convert {
                println!("[subtitle] Smart mode: Converting text-based subtitles to WebVTT");
                cmd.args(["-map", "0:v:0"])          
                    .args(["-map", "0:a?"])          
                    .args(["-map", "0:s?"])
                    .args(["-vf", "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2"]) 
                    .args(["-c:s", "webvtt"]);
            } else {
                println!("[subtitle] Smart mode: Burning bitmap-based subtitles into video");
                cmd.args(["-filter_complex", 
                    "[0:v:0]scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2[v];[0:s:0]scale=1920:1080[s];[v][s]overlay[vout]"])
                    .args(["-map", "[vout]"])
                    .args(["-map", "0:a?"]);
            }
        }
    }

    cmd.args(["-c:v", "libx264", "-preset", "veryfast"])
        .args(["-s", "1920x1080"])
        .args(["-b:v", "5M", "-maxrate", "5M", "-bufsize", "10M"])
        .args(["-c:a", "aac", "-b:a", "128k"])
        .args(["-f", "hls"])
        .args(["-hls_time", "4"])
        .args(["-hls_list_size", "5"])
        .args([
            "-hls_flags",
            "append_list+delete_segments+program_date_time+omit_endlist+independent_segments",
        ])
        .args(["-hls_segment_filename", &seg_tmpl.to_string_lossy()])
        .arg(output_dir.join("index.m3u8"))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    cmd
}

async fn check_ffmpeg_availability() -> Result<(), String> {
    // Check cache first
    if let Some(&available) = FFMPEG_AVAILABLE.get() {
        return if available {
            Ok(())
        } else {
            Err("FFmpeg not available (cached result)".to_string())
        };
    }

    match Command::new("ffmpeg").arg("-version").output().await {
        Ok(output) => {
            if output.status.success() {
                FFMPEG_AVAILABLE.set(true).unwrap();
                Ok(())
            } else {
                FFMPEG_AVAILABLE.set(false).unwrap();
                Err(format!(
                    "FFmpeg version check failed with status: {}",
                    output.status
                ))
            }
        }
        Err(e) => {
            FFMPEG_AVAILABLE.set(false).unwrap();
            Err(format!("FFmpeg not found or not accessible: {}", e))
        }
    }
}

async fn execute_ffmpeg_streaming(cmd: &mut Command, file_path: &Path) -> Result<(), String> {
    println!("[streaming] Starting FFmpeg process...");
    println!(
        "[streaming] Working directory: {:?}",
        std::env::current_dir()
    );
    println!("[streaming] FFmpeg command: {:?}", cmd);

    match cmd.spawn() {
        Ok(mut child) => {
            let pid = child.id().unwrap_or(0);
            println!(
                "[streaming] FFmpeg process started successfully (PID: {})",
                pid
            );

            match child.wait().await {
                Ok(status) => {
                    if status.success() {
                        println!("[streaming] FFmpeg process completed successfully");
                        Ok(())
                    } else {
                        Err(format!("FFmpeg exited with error status: {}", status))
                    }
                }
                Err(e) => Err(format!("Error waiting for FFmpeg process: {}", e)),
            }
        }
        Err(e) => Err(format!(
            "Failed to start FFmpeg for {}: {}",
            file_path.display(),
            e
        )),
    }
}

/// Clean up HLS output directory before starting new streaming session
async fn cleanup_hls_directory(out_dir: &Path) -> Result<(), String> {
    if out_dir.exists() {
        println!(
            "[streaming] Cleaning up existing HLS directory: {}",
            out_dir.display()
        );
        fs::remove_dir_all(out_dir)
            .await
            .map_err(|e| format!("Failed to remove HLS directory: {}", e))?;
    }
    fs::create_dir_all(out_dir)
        .await
        .map_err(|e| format!("Failed to create HLS directory: {}", e))?;
    println!("[streaming] HLS directory ready: {}", out_dir.display());
    Ok(())
}

async fn process_episode(
    episode: &crate::models::Episode,
    _item: &crate::models::PlaylistItem,
    _played_episodes: Arc<tokio::sync::RwLock<std::collections::HashMap<String, Vec<usize>>>>,
    out_dir: &Path,
    subtitle_mode: &SubtitleMode,
) -> Result<(), String> {
    let file_path = &episode.file_path;

    if !file_path.exists() {
        return Err(format!("File does not exist: {}", file_path.display()));
    }

    check_ffmpeg_availability().await?;

    let mut cmd = build_ffmpeg_command(file_path, out_dir, subtitle_mode).await;
    execute_ffmpeg_streaming(&mut cmd, file_path).await?;

    let playlist_path = out_dir.join("index.m3u8");
    match tokio::fs::metadata(&playlist_path).await {
        Ok(metadata) => {
            println!(
                "[playlist] HLS playlist created! ({} bytes)",
                metadata.len()
            );
            Ok(())
        }
        Err(_) => Err("HLS playlist file not found after FFmpeg completion".to_string()),
    }
}

async fn process_video_file(
    file: &Path,
    out_dir: &Path,
    subtitle_mode: &SubtitleMode,
) -> Result<(), String> {
    if !file.exists() {
        return Err(format!("File does not exist: {}", file.display()));
    }

    check_ffmpeg_availability().await?;

    let mut cmd = build_ffmpeg_command(file, out_dir, subtitle_mode).await;
    execute_ffmpeg_streaming(&mut cmd, file).await?;

    println!("[tv] Streaming completed for {}", file.display());
    Ok(())
}

pub async fn start_tv_loop_if_needed(state: Arc<AppState>) {
    if state.jobs.read().await.contains_key("tv") {
        return;
    }
    let mut jobs = state.jobs.write().await;
    if jobs.contains_key("tv") {
        return;
    }

    let out_dir = state.hls_root.join("tv");
    println!("[tv] HLS output directory: {}", out_dir.display());

    if let Err(e) = cleanup_hls_directory(&out_dir).await {
        eprintln!("[tv] Failed to prepare HLS directory: {}", e);
        return;
    }

    let state_clone = Arc::clone(&state);

    let handle = tokio::spawn(async move {
        println!("[tv] Starting streaming loop - checking for content...");
        loop {
            let tv_files = {
                let guard = state_clone.tv_files.read().await;
                guard.clone()
            };

            let shows = {
                let guard = state_clone.shows.read().await;
                guard.clone()
            };

            let playlist = {
                let guard = state_clone.playlist.read().await;
                guard.clone()
            };

            let played_episodes = Arc::clone(&state_clone.played_episodes);
            let out_dir = state_clone.hls_root.join("tv");

            let subtitle_mode = {
                let guard = state_clone.subtitle_mode.read().await;
                guard.clone()
            };

            if !playlist.is_empty() {
                println!("[tv] Using playlist mode with {} items", playlist.len());
                println!("[tv] Processing playlist items...");
                println!("[tv] Playlist has {} items to process", playlist.len());
                for (i, item) in playlist.iter().enumerate() {
                    println!(
                        "[tv] Processing playlist item {}: show='{}'",
                        i, item.show_name
                    );
                    if let Some(episodes) = shows.get(&item.show_name) {
                        println!(
                            "[tv] Found {} episodes for show '{}'",
                            episodes.len(),
                            item.show_name
                        );
                        let episode_range = item.episode_range;
                        let episodes_to_play = match episode_range {
                            Some((start, end)) => {
                                let start = start.min(episodes.len());
                                let end = end.min(episodes.len());
                                &episodes[start..end]
                            }
                            None => episodes,
                        };

                        for episode in episodes_to_play {
                            if item.repeat_count == 0 {
                                let played = played_episodes.read().await;
                                if let Some(played_eps) = played.get(&item.show_name) {
                                    if played_eps.contains(&episode.id) {
                                        println!(
                                            "[playlist] Skipping already played episode: {}",
                                            episode.name
                                        );
                                        continue;
                                    }
                                }
                            }

                            if item.repeat_count == 0 {
                                played_episodes
                                    .write()
                                    .await
                                    .entry(item.show_name.clone())
                                    .or_insert_with(Vec::new)
                                    .push(episode.id);
                            }

                            println!(
                                "[playlist] Processing {} - {}",
                                item.show_name, episode.name
                            );

                            match process_episode(
                                episode,
                                item,
                                Arc::clone(&played_episodes),
                                &out_dir,
                                &subtitle_mode,
                            )
                            .await
                            {
                                Ok(_) => {
                                    println!("[playlist] Episode processed successfully");
                                }
                                Err(e) => {
                                    println!("[playlist] Failed to process episode: {}", e);
                                    time::sleep(Duration::from_secs(1)).await;
                                }
                            }
                        }
                    } else {
                        eprintln!(
                            "[tv] No episodes found for show '{}'. Please scan for videos first.",
                            item.show_name
                        );
                        time::sleep(Duration::from_secs(5)).await;
                    }
                }
            } else if !tv_files.is_empty() {
                println!("[tv] Using fallback mode with {} files", tv_files.len());
                println!("[tv] About to process {} tv_files", tv_files.len());
                for file in &tv_files {
                    println!("[tv] Processing file: {}", file.display());

                    match process_video_file(file, &out_dir, &subtitle_mode).await {
                        Ok(_) => {
                            // Success
                        }
                        Err(e) => {
                            println!("[tv] Failed to process file {}: {}", file.display(), e);
                            time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            } else {
                if playlist.is_empty() && tv_files.is_empty() {
                    eprintln!("[tv] No content available for streaming");
                    eprintln!("[tv] Please add videos to your playlist or scan for TV files");
                } else {
                    println!(
                        "[tv] Content available but not ready - Playlist: {}, TV files: {}",
                        playlist.len(),
                        tv_files.len()
                    );
                }
                time::sleep(Duration::from_secs(5)).await;
            }
        }
    });
    jobs.insert("tv".into(), handle);
}

pub async fn wait_for_file(path: &Path, timeout: Duration) -> bool {
    let start = time::Instant::now();
    loop {
        if path.exists() {
            return true;
        }
        if time::Instant::now().duration_since(start) > timeout {
            return false;
        }
        time::sleep(Duration::from_millis(200)).await;
    }
}
