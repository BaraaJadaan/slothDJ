//! The browser extension ships inside the app. SlothDJ keeps a copy in a fixed folder
//! that the browser loads with "Load unpacked", and refreshes that copy when the app
//! updates. Browsers reload unpacked extensions from disk on restart, so the extension
//! keeps up with the app without anyone downloading a zip again.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Browser {
    pub id: &'static str,
    pub name: &'static str,
    /// What to type in the address bar if the button doesn't work.
    pub url: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    pub folder: String,
    pub version: Option<String>,
    pub browsers: Vec<Browser>,
}

fn bundled_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().resource_dir().ok()?.join("extension");
    dir.join("manifest.json").is_file().then_some(dir)
}

pub fn folder(app: &AppHandle) -> PathBuf {
    app.path()
        .app_local_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("SlothDJ"))
        .join("browser-extension")
}

fn manifest_version(dir: &Path) -> Option<String> {
    let text = fs::read_to_string(dir.join("manifest.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v["version"].as_str().map(str::to_string)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Put the bundled extension into its folder (replacing an older copy) and return the folder.
pub fn prepare(app: &AppHandle) -> Result<PathBuf, String> {
    let src = bundled_dir(app).ok_or("The extension files are missing from this copy of SlothDJ. Reinstall the app.")?;
    let dest = folder(app);
    let staging = dest.with_extension("new");
    let _ = fs::remove_dir_all(&staging);
    copy_dir(&src, &staging).map_err(|e| format!("Couldn't prepare the extension folder: {e}"))?;
    // Swap in place so the browser never sees a half-copied folder.
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&staging, &dest).map_err(|e| format!("Couldn't prepare the extension folder: {e}"))?;
    Ok(dest)
}

/// After an app update, bring an existing extension folder up to date too.
pub fn refresh_if_set_up(app: &AppHandle) {
    let dest = folder(app);
    let Some(bundled) = bundled_dir(app) else { return };
    if dest.is_dir() && manifest_version(&dest) != manifest_version(&bundled) {
        let _ = prepare(app);
    }
}

pub fn setup(app: &AppHandle) -> Result<Setup, String> {
    let dir = prepare(app)?;
    Ok(Setup {
        folder: dir.to_string_lossy().into_owned(),
        version: manifest_version(&dir),
        browsers: installed().into_iter().map(|(b, _)| b).collect(),
    })
}

/// Chromium browsers we can open straight to their extensions page.
fn installed() -> Vec<(Browser, PathBuf)> {
    const KNOWN: &[(&str, &str, &str)] = &[
        ("chrome", "Chrome", "chrome://extensions"),
        ("edge", "Edge", "edge://extensions"),
        ("brave", "Brave", "brave://extensions"),
        ("opera", "Opera", "opera://extensions"),
        ("vivaldi", "Vivaldi", "vivaldi://extensions"),
    ];
    KNOWN
        .iter()
        .filter_map(|&(id, name, url)| locate(id).map(|path| (Browser { id, name, url }, path)))
        .collect()
}

#[cfg(windows)]
fn locate(id: &str) -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let (pf, pf86, local) = (env("ProgramFiles"), env("ProgramFiles(x86)"), env("LOCALAPPDATA"));
    let rel: &[&str] = match id {
        "chrome" => &[r"Google\Chrome\Application\chrome.exe"],
        "edge" => &[r"Microsoft\Edge\Application\msedge.exe"],
        "brave" => &[r"BraveSoftware\Brave-Browser\Application\brave.exe"],
        "opera" => &[r"Programs\Opera\opera.exe"],
        "vivaldi" => &[r"Vivaldi\Application\vivaldi.exe"],
        _ => &[],
    };
    [pf, pf86, local]
        .into_iter()
        .flatten()
        .flat_map(|base| rel.iter().map(move |r| base.join(r)))
        .find(|p| p.is_file())
}

#[cfg(target_os = "macos")]
fn locate(id: &str) -> Option<PathBuf> {
    let app = match id {
        "chrome" => "Google Chrome",
        "edge" => "Microsoft Edge",
        "brave" => "Brave Browser",
        "opera" => "Opera",
        "vivaldi" => "Vivaldi",
        _ => return None,
    };
    let p = PathBuf::from(format!("/Applications/{app}.app/Contents/MacOS/{app}"));
    p.is_file().then_some(p)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn locate(id: &str) -> Option<PathBuf> {
    let names: &[&str] = match id {
        "chrome" => &["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"],
        "edge" => &["microsoft-edge", "microsoft-edge-stable"],
        "brave" => &["brave-browser", "brave"],
        "opera" => &["opera"],
        "vivaldi" => &["vivaldi", "vivaldi-stable"],
        _ => &[],
    };
    names.iter().find_map(|n| crate::util::which(n))
}

/// Open a browser on its extensions page (browsers only accept these pages from their own command line).
pub fn open_extensions_page(id: &str) -> Result<(), String> {
    let (browser, path) = installed()
        .into_iter()
        .find(|(b, _)| b.id == id)
        .ok_or("That browser isn't installed.")?;
    Command::new(path)
        .arg(browser.url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't open {}: {e}", browser.name))
}
