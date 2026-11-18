// slothDJ: slow animal, fast downloads.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod engine;
mod jobs;
mod search;
mod server;
mod soundcloud;
mod sources;
mod state;
mod util;
mod youtube;

use state::lock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_deep_link::DeepLinkExt;

/// Set when the user picks Quit, so closing the window really exits instead of hiding.
static QUITTING: AtomicBool = AtomicBool::new(false);

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn quit(app: &AppHandle) {
    QUITTING.store(true, Ordering::SeqCst);
    app.exit(0);
}

/// `slothdj://download?url=<link>&format=mp3_320`, opened by the extension
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

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open slothDJ", true, None::<&str>)?;
    let folder = MenuItem::with_id(app, "folder", "Open download folder", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit slothDJ", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &folder, &sep, &quit_item])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("slothDJ")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "folder" => {
                let _ = commands::open_output_dir(app.clone());
            }
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Look for a new slothDJ a few seconds after launch, then every six hours.
fn start_update_checks(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(8));
        if let Ok(Some(info)) = tauri::async_runtime::block_on(commands::check_update(app.clone())) {
            let _ = app.emit("update-available", info);
        }
        std::thread::sleep(Duration::from_secs(6 * 60 * 60));
    });
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| show_main(app)))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let handle = app.handle().clone();
            let shared = state::AppState::load(&handle);
            app.manage(shared.clone());

            // Keep the "start with system" switch honest if it was changed outside the app.
            {
                use tauri_plugin_autostart::ManagerExt;
                if let Ok(on) = app.autolaunch().is_enabled() {
                    lock(&shared.settings).start_with_system = on;
                }
            }

            // Release builds own the slothdj:// scheme. Dev builds leave it alone so they
            // don't hijack an installed copy's links.
            #[cfg(all(any(windows, target_os = "linux"), not(debug_assertions)))]
            if let Err(e) = app.deep_link().register_all() {
                eprintln!("slothDJ: couldn't register slothdj:// links: {e}");
            }
            let h = handle.clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    handle_link(&h, url.as_str());
                }
            });
            let launched_by_link = matches!(app.deep_link().get_current(), Ok(Some(ref urls)) if !urls.is_empty());
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    handle_link(&handle, url.as_str());
                }
            }

            build_tray(&handle)?;

            // Started at login: stay in the tray. Otherwise show the window.
            let hidden = std::env::args().any(|a| a == "--hidden");
            if !hidden || launched_by_link {
                show_main(&handle);
            }

            server::start(handle.clone());
            engine::ensure(handle.clone(), false);
            start_update_checks(handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let st = state::state(app);
                let to_tray = lock(&st.settings).close_to_tray;
                if to_tray && !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                    // The first time, say where slothDJ went.
                    let first = !std::mem::replace(&mut lock(&st.settings).tray_hint_shown, true);
                    if first {
                        st.save_settings();
                        use tauri_plugin_notification::NotificationExt;
                        let _ = app
                            .notification()
                            .builder()
                            .title("slothDJ is still running")
                            .body("It keeps working in the tray so the browser extension can reach it. Right-click the sloth icon to quit.")
                            .show();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::add_download,
            commands::preview,
            commands::check_url,
            commands::search,
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
            commands::set_autostart,
            commands::update_engine,
            commands::repair_engine,
            commands::read_clipboard,
            commands::check_update,
            commands::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while starting slothDJ");

    app.run(|app, event| match event {
        RunEvent::Exit => {
            // Don't leave yt-dlp/ffmpeg running in the background after quitting.
            let pids: Vec<u32> = lock(&state::state(app).pids).values().copied().collect();
            for pid in pids {
                util::kill_tree(pid);
            }
        }
        // macOS: clicking the Dock icon brings the hidden window back.
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_main(app),
        _ => {}
    });
}
