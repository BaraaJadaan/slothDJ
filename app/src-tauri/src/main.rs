// slothDJ: slow animal, fast downloads.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod engine;
mod jobs;
mod server;
mod soundcloud;
mod state;
mod util;

use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager, RunEvent};
use tauri_plugin_deep_link::DeepLinkExt;

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// `slothdj://download?url=<soundcloud link>&format=mp3_320`, opened by the extension
/// when the app isn't running yet.
fn handle_link(app: &AppHandle, link: &str) {
    let Ok(u) = url::Url::parse(link) else { return };
    if u.scheme() != "slothdj" {
        return;
    }
    let q: HashMap<String, String> = u.query_pairs().into_owned().collect();
    if let Some(target) = q.get("url") {
        if let Err(e) = jobs::enqueue(app, target, q.get("format").map(String::as_str), "extension") {
            let _ = app.emit("toast", e);
        }
    }
    show_main(app);
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| show_main(app)))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(state::AppState::load(&handle));

            #[cfg(any(windows, target_os = "linux"))]
            if let Err(e) = app.deep_link().register_all() {
                eprintln!("slothDJ: couldn't register slothdj:// links: {e}");
            }
            let h = handle.clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    handle_link(&h, url.as_str());
                }
            });
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    handle_link(&handle, url.as_str());
                }
            }

            server::start(handle.clone());
            engine::ensure(handle, false);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::add_download,
            commands::preview,
            commands::check_url,
            commands::cancel_job,
            commands::retry_job,
            commands::remove_job,
            commands::clear_finished,
            commands::open_path,
            commands::reveal_path,
            commands::open_output_dir,
            commands::open_url,
            commands::pick_output_dir,
            commands::save_settings,
            commands::update_engine,
            commands::repair_engine,
            commands::read_clipboard,
        ])
        .build(tauri::generate_context!())
        .expect("error while starting slothDJ");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            // Don't leave yt-dlp/ffmpeg running in the background after quitting.
            let pids: Vec<u32> = state::lock(&state::state(app).pids).values().copied().collect();
            for pid in pids {
                util::kill_tree(pid);
            }
        }
    });
}
