//! Commands the UI calls through `invoke`.

use crate::engine::{self, EngineInfo};
use crate::jobs::{self, Format, Job, FORMATS};
use crate::search::{self, Hit};
use crate::sources::{self, Kind, Preview, Site};
use crate::state::{lock, state, Settings};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    version: &'static str,
    /// The extension version this app ships, to tell whether the browser's copy is behind.
    extension_latest: Option<String>,
    platform: &'static str,
    port: Option<u16>,
    settings: Settings,
    jobs: Vec<Job>,
    engine: EngineInfo,
    formats: &'static [Format],
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
        extension_latest: crate::extension::bundled_version(&app),
        platform: std::env::consts::OS,
        port,
        settings,
        jobs,
        engine,
        formats: FORMATS,
    }
}

#[tauri::command]
pub fn add_download(app: AppHandle, url: String, format: Option<String>) -> Result<Job, String> {
    jobs::enqueue(&app, &url, format.as_deref(), "app")
}

#[tauri::command]
pub async fn preview(url: String) -> Result<Preview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target = sources::parse(&url)?;
        sources::preview(&target)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInfo {
    url: String,
    site: Site,
    kind: Kind,
}

/// Lightweight check used while typing: is this a link we can download, and from where?
#[tauri::command]
pub fn check_url(url: String) -> Result<LinkInfo, String> {
    sources::parse(&url).map(|t| LinkInfo { url: t.url, site: t.site, kind: t.kind })
}

/// Prepare the extension folder for "Load unpacked", copy its path, and list browsers we can open.
#[tauri::command]
pub fn extension_setup(app: AppHandle) -> Result<crate::extension::Setup, String> {
    let setup = crate::extension::setup(&app)?;
    let _ = app.clipboard().write_text(setup.folder.clone());
    Ok(setup)
}

#[tauri::command]
pub fn open_extensions_page(browser: String) -> Result<(), String> {
    crate::extension::open_extensions_page(&browser)
}

#[tauri::command]
pub fn reveal_extension_folder(app: AppHandle) -> Result<(), String> {
    let dir = crate::extension::folder(&app);
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

/// Something playable for a link: a small YouTube stream or SoundCloud's embedded player.
#[tauri::command]
pub async fn preview_stream(app: AppHandle, url: String) -> Result<crate::stream::Stream, String> {
    tauri::async_runtime::spawn_blocking(move || crate::stream::preview_stream(&app, &url))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn search(app: AppHandle, query: String) -> Result<Vec<Hit>, String> {
    tauri::async_runtime::spawn_blocking(move || search::search(&app, &query))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_job(app: AppHandle, id: u64) {
    jobs::cancel(&app, id);
}

#[tauri::command]
pub fn retry_job(app: AppHandle, id: u64) {
    jobs::retry(&app, id);
}

/// Remove from the list; with `delete_files`, also move the downloaded file(s) to the Recycle Bin.
/// Returns how many files were deleted.
#[tauri::command]
pub fn remove_job(app: AppHandle, id: u64, delete_files: Option<bool>) -> Result<usize, String> {
    if delete_files.unwrap_or(false) {
        jobs::delete_with_files(&app, id)
    } else {
        jobs::remove(&app, id);
        Ok(0)
    }
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
    let allowed = ["https://github.com/BaraaJadaan/slothDJ", "https://soundcloud.com/", "https://www.youtube.com/"];
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
        .set_title("Choose where SlothDJ saves downloads")
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
        let out = if settings.output_dir.trim().is_empty() { s.output_dir.clone() } else { settings.output_dir };
        let format = if jobs::format_info(&settings.format).is_some() { settings.format } else { s.format.clone() };
        *s = Settings {
            output_dir: out,
            format,
            max_concurrent: settings.max_concurrent.clamp(1, 4),
            // These are owned by the app, not the settings form.
            last_update_check: s.last_update_check,
            start_with_system: s.start_with_system,
            tray_hint_shown: s.tray_hint_shown,
            extension_seen: s.extension_seen,
            extension_version: s.extension_version.clone(),
            remove_action: if ["ask", "list", "file"].contains(&settings.remove_action.as_str()) {
                settings.remove_action.clone()
            } else {
                s.remove_action.clone()
            },
            ..settings
        };
        s.clone()
    };
    st.save_settings();
    jobs::pump(&app);
    saved
}

/// Turn "start with Windows/macOS/Linux" on or off. Returns the real state afterwards.
#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<Settings, String> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| format!("Couldn't change the startup setting: {e}"))?;
    let st = state(&app);
    lock(&st.settings).start_with_system = launcher.is_enabled().unwrap_or(enabled);
    st.save_settings();
    let s = lock(&st.settings).clone();
    Ok(s)
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    notes: Option<String>,
}

/// Ask GitHub whether a newer SlothDJ exists. Remembers it for `install_update`.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|_| "Couldn't check for updates. Check your connection.".to_string())?;
    let info = update.as_ref().map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone() });
    *lock(&state(&app).update) = update;
    Ok(info)
}

/// Download the update found by `check_update`, install it and restart SlothDJ.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let Some(update) = lock(&state(&app).update).take() else {
        return Err("No update to install. Check again.".into());
    };
    let progress = app.clone();
    let mut received = 0u64;
    update
        .download_and_install(
            move |chunk, total| {
                received += chunk as u64;
                let _ = progress.emit("update-progress", (received, total));
            },
            || {},
        )
        .await
        .map_err(|e| format!("The update couldn't be installed: {e}"))?;
    // Stop running downloads cleanly before the new version starts.
    let pids: Vec<u32> = lock(&state(&app).pids).values().copied().collect();
    for pid in pids {
        crate::util::kill_tree(pid);
    }
    app.restart();
}
