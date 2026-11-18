//! Shared app state, settings and on-disk locations.

use crate::engine::Engine;
use crate::jobs::Job;
use crate::util::{read_json, write_json};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Manager};

pub type Shared = Arc<AppState>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub output_dir: String,
    pub format: String,
    pub embed_artwork: bool,
    pub max_concurrent: u8,
    pub theme: String,
    pub last_update_check: u64,
    /// Show a system notification when a download finishes in the background.
    pub notify: bool,
    /// Closing the window keeps slothDJ running in the tray.
    pub close_to_tray: bool,
    /// Mirrors the OS login item; changed through `set_autostart`, not `save_settings`.
    pub start_with_system: bool,
    /// The "still running in the tray" tip has been shown once.
    pub tray_hint_shown: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: String::new(),
            format: "mp3_320".into(),
            embed_artwork: true,
            max_concurrent: 2,
            theme: "system".into(),
            last_update_check: 0,
            notify: true,
            close_to_tray: true,
            start_with_system: false,
            tray_hint_shown: false,
        }
    }
}

pub struct Paths {
    pub bin_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub settings_file: PathBuf,
    pub history_file: PathBuf,
}

pub struct AppState {
    pub paths: Paths,
    pub settings: Mutex<Settings>,
    pub jobs: Mutex<Vec<Job>>,
    pub engine: Mutex<Engine>,
    pub next_id: AtomicU64,
    pub port: Mutex<Option<u16>>,
    /// Running yt-dlp processes by job id, so they can be cancelled.
    pub pids: Mutex<HashMap<u64, u32>>,
    pub cancelled: Mutex<HashSet<u64>>,
    /// An update found by the updater, waiting for the user to install it.
    pub update: Mutex<Option<tauri_plugin_updater::Update>>,
}

impl AppState {
    pub fn load(app: &AppHandle) -> Shared {
        let path = app.path();
        let data = path.app_local_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("slothDJ"));
        let config = path.app_config_dir().unwrap_or_else(|_| data.clone());
        let paths = Paths {
            bin_dir: data.join("bin"),
            temp_dir: data.join("tmp"),
            settings_file: config.join("settings.json"),
            history_file: data.join("history.json"),
        };
        // Leftovers from downloads that were interrupted last time.
        let _ = std::fs::remove_dir_all(&paths.temp_dir);

        let mut settings: Settings = read_json(&paths.settings_file).unwrap_or_default();
        if settings.output_dir.is_empty() {
            let base = path
                .audio_dir()
                .or_else(|_| path.download_dir())
                .or_else(|_| path.home_dir())
                .unwrap_or_else(|_| PathBuf::from("."));
            settings.output_dir = base.join("slothDJ").to_string_lossy().into_owned();
        }

        let mut jobs: Vec<Job> = read_json(&paths.history_file).unwrap_or_default();
        // Anything still in flight when the app last closed shows up as canceled, ready to retry.
        for j in jobs.iter_mut().filter(|j| !j.status.is_finished()) {
            j.status = crate::jobs::Status::Canceled;
            j.speed = None;
            j.eta = None;
        }
        jobs.sort_by_key(|j| j.created);
        let next_id = jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1;

        Arc::new(AppState {
            engine: Mutex::new(Engine::new(&paths.bin_dir)),
            paths,
            settings: Mutex::new(settings),
            jobs: Mutex::new(jobs),
            next_id: AtomicU64::new(next_id),
            port: Mutex::new(None),
            pids: Mutex::new(HashMap::new()),
            cancelled: Mutex::new(HashSet::new()),
            update: Mutex::new(None),
        })
    }

    pub fn save_settings(&self) {
        let s = lock(&self.settings).clone();
        write_json(&self.paths.settings_file, &s);
    }
}

/// Grab the shared state from anywhere that has an `AppHandle`.
pub fn state(app: &AppHandle) -> Shared {
    app.state::<Shared>().inner().clone()
}

/// `Mutex::lock` that shrugs off poisoning (a panicked thread shouldn't brick the app).
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
