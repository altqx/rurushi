use std::{ collections::HashMap, path::PathBuf, sync::Arc };

use serde::{ Deserialize, Serialize };
use tokio::{ sync::RwLock, task::JoinHandle };

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SubtitleMode {
    None,
    Smart,
}

impl Default for SubtitleMode {
    fn default() -> Self {
        SubtitleMode::None
    }
}

pub struct AppState {
    pub tv_files: RwLock<Vec<PathBuf>>,
    pub hls_root: PathBuf,
    pub jobs: RwLock<HashMap<String, JoinHandle<()>>>,
    pub videos_folder: RwLock<Option<PathBuf>>,
    pub shows: RwLock<HashMap<String, Vec<Episode>>>,
    pub playlist: RwLock<Vec<PlaylistItem>>,
    pub played_episodes: Arc<RwLock<HashMap<String, Vec<usize>>>>,
    pub subtitle_mode: RwLock<SubtitleMode>,
    pub current_playing: RwLock<Option<PathBuf>>,
    pub is_playing: RwLock<bool>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Episode {
    pub id: usize,
    pub name: String,
    pub file_path: PathBuf,
    pub show_name: String,
    pub episode_number: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PlaylistItem {
    pub show_name: String,
    pub episode_range: Option<(usize, usize)>,
    pub repeat_count: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub videos_folder: Option<PathBuf>,
    pub shows: HashMap<String, Vec<Episode>>,
    pub playlist: Vec<PlaylistItem>,
    pub played_episodes: HashMap<String, Vec<usize>>,
    pub subtitle_mode: SubtitleMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            videos_folder: None,
            shows: HashMap::new(),
            playlist: Vec::new(),
            played_episodes: HashMap::new(),
            subtitle_mode: SubtitleMode::default(),
        }
    }
}
