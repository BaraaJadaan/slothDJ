//! The download engine: yt-dlp (extraction) + ffmpeg (conversion and tagging).
//!
//! Neither is bundled, which keeps the installer tiny. On first launch SlothDJ fetches
//! the official yt-dlp build and a static ffmpeg (or reuses one already on PATH), then
//! keeps yt-dlp current, since sites change and yt-dlp ships fixes quickly.

use crate::state::{lock, state};
use crate::util::{command, exe_name, http, now_ms, which};
use flate2::read::GzDecoder;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
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
    /// "system" when an ffmpeg on PATH is reused, "managed" when SlothDJ downloaded it.
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
/// on PATH, or a Deno that SlothDJ downloaded earlier.
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
pub fn js_runtime(app: &AppHandle, job_id: Option<u64>) -> Result<Option<(String, PathBuf)>, String> {
    let _one_at_a_time = JS_SETUP.lock().unwrap_or_else(|e| e.into_inner());
    let st = state(app);
    if let Some(rt) = lock(&st.engine).js.clone().filter(|(_, p)| p.is_file()) {
        return Ok(Some(rt));
    }
    let (rt, source) = match existing_js(&st.paths.bin_dir) {
        Some(found) => found,
        None => {
            if let Some(id) = job_id {
                crate::jobs::update(app, id, |j| j.note = Some("Setting up YouTube support (one-time, about 40 MB)…".into()));
            }
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

const DOWNLOAD_ATTEMPTS: u64 = 5;

/// Download `url` to `dest`. GitHub's download servers sometimes drop a connection
/// mid-way, so a failed attempt picks up where it stopped (HTTP Range) instead of
/// starting over. `.gz` files are unpacked from disk once the download is complete.
fn download(app: &AppHandle, url: &str, dest: &Path, label: &str, gunzip: bool) -> Result<(), String> {
    let part = dest.with_extension("part");
    let _ = fs::remove_file(&part);
    let mut failure = None;
    for attempt in 1..=DOWNLOAD_ATTEMPTS {
        let mut report = |done: u64, total: Option<u64>| {
            let mb = |b: u64| b as f64 / 1_048_576.0;
            publish(app, |i| {
                i.stage = match total {
                    Some(t) => format!("{label} · {:.0} of {:.0} MB", mb(done), mb(t)),
                    None => format!("{label} · {:.0} MB", mb(done)),
                };
                i.progress = total.map(|t| (done as f64 / t as f64).min(1.0)).unwrap_or(0.0);
            });
        };
        match fetch_into(url, &part, &mut report) {
            Ok(()) => {
                failure = None;
                break;
            }
            Err(reason) => {
                failure = Some(reason);
                if attempt < DOWNLOAD_ATTEMPTS {
                    publish(app, |i| i.stage = format!("{label} · connection dropped, resuming…"));
                    std::thread::sleep(Duration::from_secs(attempt));
                }
            }
        }
    }
    if let Some(reason) = failure {
        let _ = fs::remove_file(&part);
        return Err(format!("Couldn't download the engine ({reason}). Check your internet connection and try again."));
    }

    let finished = if gunzip {
        let unpack = || -> std::io::Result<()> {
            let tmp = dest.with_extension("unpacking");
            let mut out = File::create(&tmp)?;
            std::io::copy(&mut GzDecoder::new(File::open(&part)?), &mut out)?;
            out.flush()?;
            drop(out);
            fs::rename(&tmp, dest)
        };
        unpack().map_err(|e| format!("The download was damaged ({e}). Try again."))
    } else {
        fs::rename(&part, dest).map_err(|e| e.to_string())
    };
    let _ = fs::remove_file(&part);
    finished?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dest, fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

/// A short, human reason for a network error ("timed out", "connection reset", …).
fn short_reason(err: &str) -> String {
    let lower = err.to_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") {
        "timed out".into()
    } else if lower.contains("reset") || lower.contains("aborted") || lower.contains("broken pipe") {
        "connection reset".into()
    } else if lower.contains("dns") || lower.contains("resolve") || lower.contains("lookup") {
        "no internet connection".into()
    } else {
        err.chars().take(80).collect()
    }
}

/// One attempt: continue `part` from its current size until the server's file is complete.
/// `progress(done, total)` is called a few times a second.
fn fetch_into(url: &str, part: &Path, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<(), String> {
    let have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let mut request = http().get(url);
    if have > 0 {
        request = request.set("Range", &format!("bytes={have}-"));
    }
    let resp = match request.call() {
        Ok(r) => r,
        Err(ureq::Error::Status(416, _)) => {
            // The server won't resume this one: start the file over next time.
            let _ = fs::remove_file(part);
            return Err("download restarted".into());
        }
        Err(ureq::Error::Status(code, _)) => return Err(format!("server answered {code}")),
        Err(e) => return Err(short_reason(&e.to_string())),
    };
    let resumed = have > 0 && resp.status() == 206;
    let start = if resumed { have } else { 0 };
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).map(|n| n + start);
    let mut file = if resumed { fs::OpenOptions::new().append(true).open(part) } else { File::create(part) }
        .map_err(|e| e.to_string())?;
    let mut src = resp.into_reader();
    let mut done = start;
    let mut buf = vec![0u8; 128 * 1024];
    let mut last = Instant::now() - Duration::from_secs(1);
    loop {
        let n = src.read(&mut buf).map_err(|e| short_reason(&e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if last.elapsed() > Duration::from_millis(150) {
            last = Instant::now();
            progress(done, total);
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    match total {
        Some(t) if done < t => Err("connection closed early".into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server that promises 1 MB, hangs up after 300 KB, then serves the rest on a Range request.
    #[test]
    fn resumes_after_a_dropped_connection() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let body: Vec<u8> = (0..1_000_000u32).map(|i| (i % 251) as u8).collect();
        let served = body.clone();
        std::thread::spawn(move || {
            for (n, req) in server.incoming_requests().enumerate() {
                if n == 0 {
                    let mut raw = req.into_writer();
                    let _ = write!(raw, "HTTP/1.1 200 OK
Content-Length: {}
Connection: close

", served.len());
                    let _ = raw.write_all(&served[..300_000]);
                    let _ = raw.flush();
                } else {
                    let range = req.headers().iter().find(|h| h.field.equiv("Range")).map(|h| h.value.to_string()).unwrap();
                    let from: usize = range.trim_start_matches("bytes=").trim_end_matches('-').parse().unwrap();
                    let resp = tiny_http::Response::from_data(served[from..].to_vec()).with_status_code(206);
                    let _ = req.respond(resp);
                }
            }
        });

        let dir = std::env::temp_dir().join(format!("slothdj-resume-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let part = dir.join("file.part");
        let url = format!("http://127.0.0.1:{port}/file");

        assert!(fetch_into(&url, &part, &mut |_, _| {}).is_err(), "first attempt is cut off");
        assert_eq!(fs::metadata(&part).unwrap().len(), 300_000, "keeps what arrived");
        fetch_into(&url, &part, &mut |_, _| {}).expect("second attempt resumes");
        assert_eq!(fs::read(&part).unwrap(), body, "the resumed file is identical");
        let _ = fs::remove_dir_all(&dir);
    }
}
