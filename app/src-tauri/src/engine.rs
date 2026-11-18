//! The download engine: yt-dlp (extraction) + ffmpeg (conversion and tagging).
//!
//! Neither is bundled, which keeps the installer tiny. On first launch slothDJ fetches
//! the official yt-dlp build and a static ffmpeg (or reuses one already on PATH), then
//! keeps yt-dlp current, since sites change and yt-dlp ships fixes quickly.

use crate::state::{lock, state};
use crate::util::{command, exe_name, http, now_ms, which};
use flate2::read::GzDecoder;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const UPDATE_EVERY_MS: u64 = 2 * 24 * 60 * 60 * 1000;
const FFMPEG_RELEASE: &str = "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1";

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub ready: bool,
    pub busy: bool,
    pub stage: String,
    pub progress: f64,
    pub ytdlp_version: Option<String>,
    /// "system" when an ffmpeg on PATH is reused, "managed" when slothDJ downloaded it.
    pub ffmpeg: Option<String>,
    /// JavaScript runtime used for YouTube, e.g. "node (system)" or "deno (managed)".
    pub js: Option<String>,
    pub error: Option<String>,
}

pub struct Engine {
    pub ytdlp: PathBuf,
    pub ffmpeg: Option<PathBuf>,
    /// ("node" | "deno", path), found or downloaded on first YouTube use.
    pub js: Option<(String, PathBuf)>,
    pub info: EngineInfo,
}

impl Engine {
    pub fn new(bin_dir: &Path) -> Self {
        Self { ytdlp: bin_dir.join(exe_name("yt-dlp")), ffmpeg: None, js: None, info: EngineInfo::default() }
    }
}

fn ytdlp_asset() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else if cfg!(target_arch = "aarch64") {
        "yt-dlp_linux_aarch64"
    } else {
        "yt-dlp_linux"
    }
}

fn ffmpeg_asset() -> String {
    let os = if cfg!(windows) { "win32" } else if cfg!(target_os = "macos") { "darwin" } else { "linux" };
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    format!("ffmpeg-{os}-{arch}.gz")
}

fn publish(app: &AppHandle, f: impl FnOnce(&mut EngineInfo)) {
    let st = state(app);
    let info = {
        let mut e = lock(&st.engine);
        f(&mut e.info);
        e.info.clone()
    };
    let _ = app.emit("engine", info);
}

/// Make sure yt-dlp and ffmpeg are present (downloading them if needed). Runs in the background.
pub fn ensure(app: AppHandle, force_update: bool) {
    std::thread::spawn(move || {
        if lock(&state(&app).engine).info.busy {
            return;
        }
        publish(&app, |i| {
            // Not "ready" while binaries may be swapped out; queued jobs wait.
            i.ready = false;
            i.busy = true;
            i.error = None;
            i.stage = "Checking engine…".into();
            i.progress = 0.0;
        });
        match prepare(&app, force_update) {
            Ok(()) => {
                publish(&app, |i| {
                    i.busy = false;
                    i.ready = true;
                    i.stage = String::new();
                });
                crate::jobs::pump(&app);
            }
            Err(err) => publish(&app, |i| {
                i.busy = false;
                i.error = Some(err);
                i.stage = String::new();
            }),
        }
    });
}

fn prepare(app: &AppHandle, force_update: bool) -> Result<(), String> {
    let st = state(app);
    fs::create_dir_all(&st.paths.bin_dir).map_err(|e| format!("Can't create engine folder: {e}"))?;

    // yt-dlp
    let ytdlp = lock(&st.engine).ytdlp.clone();
    let mut fresh = false;
    if !ytdlp.is_file() {
        let url = format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{}", ytdlp_asset());
        download(app, &url, &ytdlp, "Downloading yt-dlp", false)?;
        fresh = true;
    }

    // ffmpeg: reuse a system one when available, it saves a 30 MB download.
    let managed = st.paths.bin_dir.join(exe_name("ffmpeg"));
    let (ffmpeg, source) = if managed.is_file() {
        (managed, "managed")
    } else if let Some(sys) = which("ffmpeg") {
        (sys, "system")
    } else {
        download(app, &format!("{FFMPEG_RELEASE}/{}", ffmpeg_asset()), &managed, "Downloading ffmpeg", true)?;
        (managed, "managed")
    };
    lock(&st.engine).ffmpeg = Some(ffmpeg);
    publish(app, |i| i.ffmpeg = Some(source.into()));

    // Keep yt-dlp fresh: sites change, yt-dlp ships fixes within days.
    let last = lock(&st.settings).last_update_check;
    if !fresh && (force_update || now_ms().saturating_sub(last) > UPDATE_EVERY_MS) {
        publish(app, |i| {
            i.stage = "Updating yt-dlp…".into();
            i.progress = 0.0;
        });
        let _ = command(&ytdlp).arg("-U").output();
    }
    if fresh || force_update || now_ms().saturating_sub(last) > UPDATE_EVERY_MS {
        lock(&st.settings).last_update_check = now_ms();
        st.save_settings();
    }

    let out = command(&ytdlp)
        .arg("--version")
        .output()
        .map_err(|e| format!("yt-dlp won't start: {e}"))?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if version.is_empty() {
        return Err("yt-dlp won't start. Try “Repair engine” in Settings.".into());
    }
    publish(app, |i| i.ytdlp_version = Some(version));

    // Note an existing JavaScript runtime now, so Settings can show it; nothing is downloaded here.
    if let Some((rt, source)) = existing_js(&st.paths.bin_dir) {
        lock(&st.engine).js = Some(rt.clone());
        publish(app, |i| i.js = Some(format!("{} ({source})", rt.0)));
    }
    Ok(())
}

static JS_SETUP: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `exe --version` is at least `major.minor` (handles "v24.1.0" and "deno 2.9.8 (…)").
fn version_at_least(exe: &Path, major: u64, minor: u64) -> bool {
    let Ok(out) = command(exe).arg("--version").output() else { return false };
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.split(|c: char| !(c.is_ascii_digit() || c == '.')).find(|s| s.contains('.'));
    let mut parts = first.unwrap_or_default().split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0)) >= (major, minor)
}

/// A JavaScript runtime yt-dlp can use, already on this computer: Deno 2.3+ or Node 22+
/// on PATH, or a Deno that slothDJ downloaded earlier.
fn existing_js(bin_dir: &Path) -> Option<((String, PathBuf), &'static str)> {
    if let Some(p) = which("deno").filter(|p| version_at_least(p, 2, 3)) {
        return Some((("deno".into(), p), "system"));
    }
    if let Some(p) = which("node").filter(|p| version_at_least(p, 22, 0)) {
        return Some((("node".into(), p), "system"));
    }
    let managed = bin_dir.join(exe_name("deno"));
    managed.is_file().then(|| (("deno".into(), managed), "managed"))
}

fn deno_asset() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "aarch64") => "deno-aarch64-pc-windows-msvc.zip",
        ("windows", _) => "deno-x86_64-pc-windows-msvc.zip",
        ("macos", "aarch64") => "deno-aarch64-apple-darwin.zip",
        ("macos", _) => "deno-x86_64-apple-darwin.zip",
        (_, "aarch64") => "deno-aarch64-unknown-linux-gnu.zip",
        _ => "deno-x86_64-unknown-linux-gnu.zip",
    }
}

/// The runtime YouTube downloads use. The first YouTube job on a computer without
/// Node or Deno downloads Deno once (about 40 MB), showing progress on that job.
pub fn js_runtime(app: &AppHandle, job_id: u64) -> Result<Option<(String, PathBuf)>, String> {
    let _one_at_a_time = JS_SETUP.lock().unwrap_or_else(|e| e.into_inner());
    let st = state(app);
    if let Some(rt) = lock(&st.engine).js.clone().filter(|(_, p)| p.is_file()) {
        return Ok(Some(rt));
    }
    let (rt, source) = match existing_js(&st.paths.bin_dir) {
        Some(found) => found,
        None => {
            crate::jobs::update(app, job_id, |j| j.note = Some("Setting up YouTube support (one-time, about 40 MB)…".into()));
            let managed = st.paths.bin_dir.join(exe_name("deno"));
            let zip_path = st.paths.bin_dir.join("deno.zip");
            let url = format!("https://github.com/denoland/deno/releases/latest/download/{}", deno_asset());
            publish(app, |i| i.busy = true);
            let fetched = download(app, &url, &zip_path, "Downloading Deno for YouTube", false)
                .and_then(|()| extract_one(&zip_path, &exe_name("deno"), &managed));
            let _ = fs::remove_file(&zip_path);
            publish(app, |i| {
                i.busy = false;
                i.stage = String::new();
            });
            fetched.map_err(|_| "Couldn't set up YouTube support. Check your connection and retry.".to_string())?;
            (("deno".to_string(), managed), "managed")
        }
    };
    lock(&st.engine).js = Some(rt.clone());
    publish(app, |i| i.js = Some(format!("{} ({source})", rt.0)));
    Ok(Some(rt))
}

/// Pull one file out of a .zip (the Deno release is a zip holding just the binary).
fn extract_one(zip_path: &Path, name: &str, dest: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut entry = archive.by_name(name).map_err(|e| e.to_string())?;
    let tmp = dest.with_extension("extracting");
    let mut out = File::create(&tmp).map_err(|e| e.to_string())?;
    std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    drop(out);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
    }
    fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

/// Delete the engine binaries and fetch them again.
pub fn repair(app: AppHandle) {
    let st = state(&app);
    let _ = fs::remove_file(st.paths.bin_dir.join(exe_name("yt-dlp")));
    let _ = fs::remove_file(st.paths.bin_dir.join(exe_name("ffmpeg")));
    lock(&st.engine).info.ready = false;
    ensure(app, false);
}

struct Counting<R> {
    inner: R,
    count: Arc<AtomicU64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// Download with a few retries: big files from GitHub's CDN occasionally drop mid-way.
fn download(app: &AppHandle, url: &str, dest: &Path, label: &str, gunzip: bool) -> Result<(), String> {
    let mut last_err = String::new();
    for attempt in 1..=3 {
        match download_once(app, url, dest, label, gunzip) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let _ = fs::remove_file(dest.with_extension("part"));
                last_err = e;
                if attempt < 3 {
                    publish(app, |i| i.stage = format!("{label} · retrying ({attempt} of 2)…"));
                    std::thread::sleep(Duration::from_secs(2 * attempt));
                }
            }
        }
    }
    Err(last_err)
}

fn download_once(app: &AppHandle, url: &str, dest: &Path, label: &str, gunzip: bool) -> Result<(), String> {
    let offline = || "Couldn't download the engine. Check your internet connection and try again.".to_string();
    let resp = http().get(url).call().map_err(|_| offline())?;
    let total: f64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let count = Arc::new(AtomicU64::new(0));
    let counted = Counting { inner: resp.into_reader(), count: count.clone() };
    let mut src: Box<dyn Read> = if gunzip { Box::new(GzDecoder::new(counted)) } else { Box::new(counted) };

    let tmp = dest.with_extension("part");
    let mut file = File::create(&tmp).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 128 * 1024];
    let mut last = Instant::now() - Duration::from_secs(1);
    let mb = |b: f64| b / 1_048_576.0;
    loop {
        let n = src.read(&mut buf).map_err(|_| offline())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        if last.elapsed() > Duration::from_millis(150) {
            last = Instant::now();
            let done = count.load(Ordering::Relaxed) as f64;
            publish(app, |i| {
                i.stage = if total > 0.0 {
                    format!("{label} · {:.0} of {:.0} MB", mb(done), mb(total))
                } else {
                    format!("{label} · {:.0} MB", mb(done))
                };
                i.progress = if total > 0.0 { (done / total).min(1.0) } else { 0.0 };
            });
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
    }
    fs::rename(&tmp, dest).map_err(|e| e.to_string())
}
