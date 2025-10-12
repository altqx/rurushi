use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use regex::Regex;
use walkdir::WalkDir;

use crate::models::Episode;

pub async fn scan_for_videos(folder: &Path) -> Vec<PathBuf> {
    let mut video_files = Vec::new();
    let video_extensions = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v"];

    println!("[scan] Starting video scan of folder: {}", folder.display());
    println!("[scan] Looking for extensions: {:?}", video_extensions);

    for entry in WalkDir::new(folder).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            println!("[scan] Found file: {}", path.display());

            if let Some(extension) = path.extension() {
                if let Some(ext_str) = extension.to_str() {
                    let ext_lower = ext_str.to_lowercase();
                    println!("[scan] File extension: {}", ext_lower);

                    if video_extensions.contains(&ext_lower.as_str()) {
                        println!("[scan] Video file accepted: {}", path.display());
                        video_files.push(path.to_path_buf());
                    } else {
                        println!(
                            "[scan] Extension '{}' not in video extensions list",
                            ext_lower
                        );
                    }
                } else {
                    println!("[scan] Could not convert extension to string");
                }
            } else {
                println!("[scan] No extension found for file: {}", path.display());
            }
        }
    }

    println!(
        "[scan] Scan complete. Found {} video files",
        video_files.len()
    );
    for (i, file) in video_files.iter().enumerate() {
        println!("[scan] {}: {}", i + 1, file.display());
    }

    video_files
}

pub async fn organize_shows_and_episodes(video_files: &[PathBuf]) -> HashMap<String, Vec<Episode>> {
    let mut shows: HashMap<String, Vec<Episode>> = HashMap::new();
    let mut episode_id = 0;

    for file_path in video_files {
        let mut episode = parse_episode_info(file_path);

        episode.id = episode_id;
        episode_id += 1;

        shows
            .entry(episode.show_name.clone())
            .or_insert_with(Vec::new)
            .push(episode);
    }

    for episodes in shows.values_mut() {
        episodes.sort_by(|a, b| match (a.episode_number, b.episode_number) {
            (Some(a_num), Some(b_num)) => a_num.cmp(&b_num),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        });

        for (index, episode) in episodes.iter_mut().enumerate() {
            episode.id = index;
        }
    }

    shows
}

fn parse_episode_info(file_path: &Path) -> Episode {
    let file_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");

    let parent_dir = file_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");

    // Extract episode number from filename patterns like:
    // "Show Name - 01", "Show Name - 02", "Show Name Episode 5", etc.
    let episode_number = extract_episode_number(file_name);

    Episode {
        id: 0, // Will be set when organizing episodes
        name: file_name.to_string(),
        file_path: file_path.to_path_buf(),
        show_name: parent_dir.to_string(),
        episode_number,
    }
}

fn extract_episode_number(filename: &str) -> Option<usize> {
    // Pattern 1: "Show Name - 01", "Show Name - 02"
    if let Some(captures) = Regex::new(r"- (\d+)").ok()?.captures(filename) {
        return captures.get(1)?.as_str().parse().ok();
    }

    // Pattern 2: "Show Name Episode 5", "Show Name Ep 10"
    if let Some(captures) = Regex::new(r"(?i)(?:episode|ep)\s+(\d+)")
        .ok()?
        .captures(filename)
    {
        return captures.get(1)?.as_str().parse().ok();
    }

    // Pattern 3: "Show Name 01", "Show Name 02" (space before number)
    if let Some(captures) = Regex::new(r"\s+(\d+)$").ok()?.captures(filename) {
        return captures.get(1)?.as_str().parse().ok();
    }

    None
}
