//! Commands the UI calls through `invoke`.

use crate::engine::{self, EngineInfo};
use crate::jobs::{self, Job, FORMATS};
use crate::soundcloud::{self, Preview};
use crate::state::{lock, state, Settings};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatOpt {
    id: &'static str,
    label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    version: &'static str,
    platform: &'static str,
    port: Option<u16>,
    settings: Settings,
    jobs: Vec<Job>,
    engine: EngineInfo,
    formats: Vec<FormatOpt>,
}

#[tauri::command]
pub fn get_state(app: AppHandle) -> Snapshot {
    let st = state(&app);
    let port = *lock(&st.port);
    let settings = lock(&st.settings).clone();
    let jobs = lock(&st.jobs).clone();
    let engine = lock(&st.engine).info.clone();
    Snapshot {
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        port,
        settings,
        jobs,
        engine,
        formats: FORMATS.iter().map(|&(id, label)| FormatOpt { id, label }).collect(),
    }
}

#[tauri::command]
pub fn add_download(app: AppHandle, url: String, format: Option<String>) -> Result<Job, String> {
    jobs::enqueue(&app, &url, format.as_deref(), "app")
}

#[tauri::command]
pub async fn preview(url: String) -> Result<Preview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target = soundcloud::parse(&url)?;
        soundcloud::preview(&target)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Lightweight check used while typing: is this a link we can download?
#[tauri::command]
pub fn check_url(url: String) -> Result<String, String> {
    soundcloud::parse(&url).map(|t| t.url)
}

#[tauri::command]
pub fn cancel_job(app: AppHandle, id: u64) {
    jobs::cancel(&app, id);
}

#[tauri::command]
pub fn retry_job(app: AppHandle, id: u64) {
    jobs::retry(&app, id);
}

#[tauri::command]
pub fn remove_job(app: AppHandle, id: u64) {
    jobs::remove(&app, id);
}

#[tauri::command]
pub fn clear_finished(app: AppHandle) {
    jobs::clear_finished(&app);
}

const MISSING: &str = "That file was moved or deleted.";

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    // Check first, so a missing file shows our message instead of a Windows error dialog.
    if !PathBuf::from(&path).exists() {
        return Err(MISSING.into());
    }
    app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    let file = PathBuf::from(&path);
    if file.exists() {
        return app.opener().reveal_item_in_dir(file).map_err(|e| e.to_string());
    }
    // The file is gone, but its folder is still the useful place to land.
    match file.parent().filter(|dir| dir.is_dir()) {
        Some(dir) => app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string()),
        None => Err(MISSING.into()),
    }
}

#[tauri::command]
pub fn open_output_dir(app: AppHandle) -> Result<(), String> {
    let dir = lock(&state(&app).settings).output_dir.clone();
    let _ = std::fs::create_dir_all(&dir);
    app.opener().open_path(dir, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    let allowed = ["https://github.com/BaraaJadaan/slothDJ", "https://soundcloud.com/"];
    if !allowed.iter().any(|p| url.starts_with(p)) {
        return Err("blocked".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_output_dir(app: AppHandle) -> Result<Settings, String> {
    let st = state(&app);
    let current = lock(&st.settings).output_dir.clone();
    let picked = app
        .dialog()
        .file()
        .set_title("Choose where slothDJ saves music")
        .set_directory(&current)
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok());
    if let Some(dir) = picked {
        lock(&st.settings).output_dir = dir.to_string_lossy().into_owned();
        st.save_settings();
    }
    let s = lock(&st.settings).clone();
    let _ = app.emit("settings", &s);
    Ok(s)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Settings {
    let st = state(&app);
    let saved = {
        let mut s = lock(&st.settings);
        let last = s.last_update_check;
        let out = if settings.output_dir.trim().is_empty() { s.output_dir.clone() } else { settings.output_dir };
        *s = Settings {
            output_dir: out,
            max_concurrent: settings.max_concurrent.clamp(1, 4),
            last_update_check: last,
            format: if FORMATS.iter().any(|(k, _)| *k == settings.format) { settings.format } else { s.format.clone() },
            ..settings
        };
        s.clone()
    };
    st.save_settings();
    jobs::pump(&app);
    saved
}

#[tauri::command]
pub fn update_engine(app: AppHandle) -> Result<(), String> {
    let busy = lock(&state(&app).jobs).iter().any(|j| j.status.is_running());
    if busy {
        return Err("Wait for running downloads to finish first.".into());
    }
    engine::ensure(app, true);
    Ok(())
}

#[tauri::command]
pub fn repair_engine(app: AppHandle) -> Result<(), String> {
    let busy = lock(&state(&app).jobs).iter().any(|j| j.status.is_running());
    if busy {
        return Err("Wait for running downloads to finish first.".into());
    }
    engine::repair(app);
    Ok(())
}

#[tauri::command]
pub fn read_clipboard(app: AppHandle) -> Option<String> {
    app.clipboard().read_text().ok().map(|s| s.trim().to_string()).filter(|s| s.len() < 2048)
}
