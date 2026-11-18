//! The download queue: one job per pasted link, run by yt-dlp with live progress.

use crate::sources::{self, Kind, Site};
use crate::state::{lock, state};
use crate::util::{command, now_ms, write_json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const HISTORY_LIMIT: usize = 300;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Format {
    pub id: &'static str,
    pub label: &'static str,
    /// Video formats only apply to YouTube.
    pub video: bool,
}

/// Output formats offered in the app and the extension.
pub const FORMATS: &[Format] = &[
    Format { id: "mp3_320", label: "MP3 · 320 kbps", video: false },
    Format { id: "mp3_192", label: "MP3 · 192 kbps", video: false },
    Format { id: "mp3_128", label: "MP3 · 128 kbps", video: false },
    Format { id: "original", label: "Original audio", video: false },
    Format { id: "mp4_best", label: "MP4 · Best quality", video: true },
    Format { id: "mp4_1080", label: "MP4 · 1080p", video: true },
    Format { id: "mp4_720", label: "MP4 · 720p", video: true },
    Format { id: "mp4_480", label: "MP4 · 480p", video: true },
];

pub fn format_info(id: &str) -> Option<&'static Format> {
    FORMATS.iter().find(|f| f.id == id)
}

/// The format a job will actually use: the requested one if it fits the site,
/// otherwise the default from Settings (and never video for SoundCloud).
pub fn effective_format(requested: Option<&str>, site: Site, default: &str) -> String {
    let fits = |id: &str| format_info(id).is_some_and(|f| !f.video || site == Site::YouTube);
    [requested.unwrap_or_default(), default]
        .into_iter()
        .find(|id| fits(id))
        .unwrap_or("mp3_320")
        .to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Queued,
    Starting,
    Downloading,
    Converting,
    Done,
    Failed,
    Canceled,
}

impl Status {
    pub fn is_running(self) -> bool {
        matches!(self, Status::Starting | Status::Downloading | Status::Converting)
    }
    pub fn is_finished(self) -> bool {
        matches!(self, Status::Done | Status::Failed | Status::Canceled)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: u64,
    pub url: String,
    /// Older history files predate YouTube support, so missing means SoundCloud.
    #[serde(default)]
    pub site: Site,
    pub kind: Kind,
    pub format: String,
    pub title: String,
    pub artist: String,
    pub artwork: Option<String>,
    pub status: Status,
    pub progress: f64,
    pub speed: Option<f64>,
    pub eta: Option<f64>,
    /// For sets/playlists: the item currently downloading.
    pub current: Option<String>,
    pub index: Option<u32>,
    pub total: Option<u32>,
    pub files: Vec<String>,
    pub error: Option<String>,
    pub warning: Option<String>,
    /// Short-lived status text, e.g. while YouTube support is being set up.
    #[serde(default)]
    pub note: Option<String>,
    pub created: u64,
    pub source: String,
}

fn emit(app: &AppHandle, job: &Job) {
    let _ = app.emit("job", job);
}

pub(crate) fn update(app: &AppHandle, id: u64, f: impl FnOnce(&mut Job)) -> Option<Job> {
    let st = state(app);
    let job = {
        let mut jobs = lock(&st.jobs);
        let job = jobs.iter_mut().find(|j| j.id == id)?;
        f(job);
        job.clone()
    };
    emit(app, &job);
    Some(job)
}

pub fn save_history(app: &AppHandle) {
    let st = state(app);
    let mut keep: Vec<Job> = lock(&st.jobs).clone();
    keep.sort_by_key(|j| std::cmp::Reverse(j.created));
    keep.truncate(HISTORY_LIMIT);
    write_json(&st.paths.history_file, &keep);
}

/// Queue a link. Returns immediately; metadata and the download follow in the background.
pub fn enqueue(app: &AppHandle, url: &str, format: Option<&str>, source: &str) -> Result<Job, String> {
    let target = sources::parse(url)?;
    let st = state(app);
    let default = lock(&st.settings).format.clone();
    let format = effective_format(format, target.site, &default);

    // Same link already queued or running? Don't download it twice.
    if let Some(existing) = lock(&st.jobs).iter().find(|j| j.url == target.url && !j.status.is_finished()) {
        return Ok(existing.clone());
    }

    let job = Job {
        id: st.next_id.fetch_add(1, Ordering::Relaxed),
        url: target.url.clone(),
        site: target.site,
        kind: target.kind,
        format,
        title: placeholder_title(&target),
        artist: String::new(),
        artwork: None,
        status: Status::Queued,
        progress: 0.0,
        speed: None,
        eta: None,
        current: None,
        index: None,
        total: None,
        files: vec![],
        error: None,
        warning: None,
        note: None,
        created: now_ms(),
        source: source.into(),
    };
    lock(&st.jobs).push(job.clone());
    emit(app, &job);
    save_history(app);

    // Fill in the real title/artist/artwork quickly via oEmbed.
    let (app2, id) = (app.clone(), job.id);
    std::thread::spawn(move || {
        if let Ok(p) = sources::preview(&target) {
            update(&app2, id, |j| {
                if j.artist.is_empty() {
                    j.title = p.title;
                    j.artist = p.artist;
                }
                j.artwork = j.artwork.take().or(p.artwork);
                j.url = p.url;
                j.kind = p.kind;
            });
        }
    });

    pump(app);
    Ok(job)
}

/// "forss/flickermood" → "Flickermood", shown until real metadata arrives.
fn placeholder_title(t: &sources::Target) -> String {
    if t.site == Site::YouTube {
        return if t.kind == Kind::Set { "YouTube playlist".into() } else { "YouTube video".into() };
    }
    pretty_slug(&t.url)
}

fn pretty_slug(url: &str) -> String {
    let slug = url.trim_end_matches('/').rsplit('/').next().unwrap_or("Track");
    let words: Vec<String> = slug
        .split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect();
    if words.is_empty() { "Track".into() } else { words.join(" ") }
}

/// Start queued jobs while there are free slots.
pub fn pump(app: &AppHandle) {
    let st = state(app);
    if !lock(&st.engine).info.ready {
        return;
    }
    let max = lock(&st.settings).max_concurrent.clamp(1, 4) as usize;
    let mut start = vec![];
    {
        let mut jobs = lock(&st.jobs);
        let free = max.saturating_sub(jobs.iter().filter(|j| j.status.is_running()).count());
        for j in jobs.iter_mut().filter(|j| j.status == Status::Queued).take(free) {
            j.status = Status::Starting;
            start.push(j.clone());
        }
    }
    for job in start {
        emit(app, &job);
        let app = app.clone();
        std::thread::spawn(move || run(app, job.id));
    }
    refresh_tray(app);
}

/// Keep the tray tooltip in sync with what's downloading.
pub fn refresh_tray(app: &AppHandle) {
    let running = lock(&state(app).jobs).iter().filter(|j| !j.status.is_finished()).count();
    if let Some(tray) = app.tray_by_id("main") {
        let tip = match running {
            0 => "slothDJ".to_string(),
            1 => "slothDJ · 1 download".to_string(),
            n => format!("slothDJ · {n} downloads"),
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}

pub fn cancel(app: &AppHandle, id: u64) {
    let st = state(app);
    let status = lock(&st.jobs).iter().find(|j| j.id == id).map(|j| j.status);
    match status {
        Some(Status::Queued) => {
            update(app, id, |j| j.status = Status::Canceled);
            refresh_tray(app);
        }
        Some(s) if s.is_running() => {
            lock(&st.cancelled).insert(id);
            if let Some(pid) = lock(&st.pids).get(&id).copied() {
                crate::util::kill_tree(pid);
            }
        }
        _ => {}
    }
}

pub fn retry(app: &AppHandle, id: u64) {
    update(app, id, |j| {
        if j.status.is_finished() {
            j.status = Status::Queued;
            j.progress = 0.0;
            j.error = None;
            j.warning = None;
            j.note = None;
            j.files.clear();
            j.current = None;
            j.index = None;
            j.speed = None;
            j.eta = None;
        }
    });
    save_history(app);
    pump(app);
}

pub fn remove(app: &AppHandle, id: u64) {
    cancel(app, id);
    lock(&state(app).jobs).retain(|j| j.id != id);
    let _ = app.emit("job-removed", id);
    save_history(app);
    refresh_tray(app);
}

pub fn clear_finished(app: &AppHandle) {
    let st = state(app);
    let removed: Vec<u64> = {
        let mut jobs = lock(&st.jobs);
        let ids = jobs.iter().filter(|j| j.status.is_finished()).map(|j| j.id).collect();
        jobs.retain(|j| !j.status.is_finished());
        ids
    };
    for id in removed {
        let _ = app.emit("job-removed", id);
    }
    save_history(app);
}

/// Everything yt-dlp needs besides the job itself.
pub(crate) struct RunCtx<'a> {
    pub out_dir: &'a str,
    pub temp_dir: &'a str,
    pub ffmpeg: &'a str,
    pub embed_art: bool,
    /// ("node" | "deno", path), used for YouTube.
    pub js: Option<(String, PathBuf)>,
}

pub(crate) fn build_args(job: &Job, ctx: &RunCtx) -> Vec<String> {
    let mut a: Vec<String> = [
        "--ignore-config", "--no-simulate", "--newline", "--progress", "--color", "never",
        "--no-mtime", "--concurrent-fragments", "4", "--retries", "5", "--fragment-retries", "10",
        // Without this, yt-dlp on Windows writes in the legacy code page and silently
        // drops characters it can't represent (e.g. Arabic) from everything it prints.
        "--encoding", "utf-8",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut push = |items: &[&str]| a.extend(items.iter().map(|s| s.to_string()));
    push(&["--ffmpeg-location", ctx.ffmpeg]);
    push(&["-P", &format!("home:{}", ctx.out_dir), "-P", &format!("temp:{}", ctx.temp_dir)]);

    if job.site == Site::YouTube {
        // YouTube's player challenges need a JavaScript runtime (see engine::js_runtime).
        if let Some((name, path)) = &ctx.js {
            push(&["--no-js-runtimes", "--js-runtimes", &format!("{name}:{}", path.to_string_lossy())]);
        }
        // "Forss - Topic" (YouTube Music channels) → "Forss", and drop "(Official Video)"-style noise.
        push(&["--replace-in-metadata", "uploader", " - Topic$", ""]);
        push(&[
            "--replace-in-metadata",
            "title",
            r"(?i)\s*[\(\[](?:official[^\)\]]*|lyrics?(?: video)?|(?:official )?audio|music video|visuali[sz]er|hd|hq|4k)[\)\]]",
            "",
        ]);
    }

    // File name: "Artist - Title", unless the title already carries an "X - Y" itself.
    push(&["--parse-metadata", "%(uploader)s - %(title)s:%(sdj_name)s"]);
    push(&["--replace-in-metadata", "sdj_name", r"^.+? - (?=.+ - )", ""]);
    if job.kind == Kind::Track {
        push(&["--no-playlist", "-o", "%(sdj_name)s.%(ext)s"]);
    } else {
        push(&["--yes-playlist", "--ignore-errors", "-o", "%(playlist_title)s/%(playlist_index)02d - %(sdj_name)s.%(ext)s"]);
    }

    // Machine-readable progress lines, parsed in `run`.
    push(&["--print", "video:SDJI %(.{title,uploader,thumbnail,playlist_index,n_entries,playlist_count,playlist_title})j"]);
    // JSON (`j`) escapes non-ASCII as \uXXXX, so the path survives any console encoding.
    push(&["--print", "after_move:SDJF %(filepath)j"]);
    push(&[
        "--progress-template",
        "download:SDJP %(progress.{status,downloaded_bytes,total_bytes,total_bytes_estimate,speed,eta,fragment_index,fragment_count})j",
    ]);

    push(&["--embed-metadata"]);
    if ctx.embed_art {
        push(&["--embed-thumbnail", "--convert-thumbnails", "jpg"]);
    }

    let video = format_info(&job.format).is_some_and(|f| f.video);
    if video {
        // H.264 + AAC plays everywhere; "best" allows VP9/AV1 when that's the only way to get 1440p/4K.
        let sort = match job.format.as_str() {
            "mp4_1080" => "res:1080,vcodec:h264,acodec:m4a",
            "mp4_720" => "res:720,vcodec:h264,acodec:m4a",
            "mp4_480" => "res:480,vcodec:h264,acodec:m4a",
            _ => "res,vcodec:h264,acodec:m4a",
        };
        push(&["-f", "bv*+ba/b", "-S", sort, "--merge-output-format", "mp4", "--embed-chapters"]);
    } else {
        if job.site == Site::YouTube && ctx.embed_art {
            // YouTube thumbnails are 16:9; music players expect square cover art.
            push(&["--ppa", "ThumbnailsConvertor+FFmpeg_o:-c:v mjpeg -vf crop=\"'if(gt(ih,iw),iw,ih)':'if(gt(iw,ih),ih,iw)'\""]);
        }
        match job.format.as_str() {
            "original" => push(&["-f", "bestaudio/best", "-x"]),
            f => {
                let kbps = f.strip_prefix("mp3_").unwrap_or("320");
                push(&["-f", "bestaudio/best", "-x", "--audio-format", "mp3", "--audio-quality", &format!("{kbps}K")]);
            }
        }
    }
    push(&["--", &job.url]);
    a
}

/// The saved file's path, printed by yt-dlp as a JSON string (see `build_args`).
fn parse_printed_path(raw: &str) -> String {
    serde_json::from_str::<String>(raw.trim()).unwrap_or_else(|_| raw.trim().to_string())
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn run(app: AppHandle, id: u64) {
    let st = state(&app);
    let Some(mut job) = lock(&st.jobs).iter().find(|j| j.id == id).cloned() else { return };

    // Short links need resolving before yt-dlp sees them.
    if job.url.starts_with("https://on.soundcloud.com") {
        match sources::parse(&job.url).and_then(sources::resolve) {
            Ok(t) => {
                job.url = t.url.clone();
                job.kind = t.kind;
                update(&app, id, |j| {
                    j.url = t.url;
                    j.kind = t.kind;
                });
            }
            Err(e) => return finish(&app, id, Err(e)),
        }
    }

    // YouTube needs a JavaScript runtime; the first time may download one.
    let js = if job.site == Site::YouTube {
        match crate::engine::js_runtime(&app, id) {
            Ok(rt) => rt,
            Err(e) => return finish(&app, id, Err(e)),
        }
    } else {
        None
    };
    update(&app, id, |j| j.note = None);

    let settings = lock(&st.settings).clone();
    let (ytdlp, ffmpeg) = {
        let e = lock(&st.engine);
        (e.ytdlp.clone(), e.ffmpeg.clone().unwrap_or_default())
    };
    let _ = std::fs::create_dir_all(&settings.output_dir);
    let temp = st.paths.temp_dir.join(id.to_string());
    let _ = std::fs::create_dir_all(&temp);
    let temp_str = temp.to_string_lossy();
    let ffmpeg_str = ffmpeg.to_string_lossy();
    let args = build_args(
        &job,
        &RunCtx {
            out_dir: &settings.output_dir,
            temp_dir: &temp_str,
            ffmpeg: &ffmpeg_str,
            embed_art: settings.embed_artwork,
            js,
        },
    );

    let child = command(&ytdlp)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return finish(&app, id, Err(format!("Couldn't start the engine: {e}"))),
    };
    lock(&st.pids).insert(id, child.id());

    // Keep the last stderr lines for a readable error message.
    let stderr = child.stderr.take().unwrap();
    let err_lines = std::thread::spawn(move || {
        let mut tail: Vec<String> = vec![];
        for raw in BufReader::new(stderr).split(b'\n').map_while(Result::ok) {
            let line = String::from_utf8_lossy(&raw).trim_end().to_string();
            if line.starts_with("ERROR:") {
                tail.push(line);
                if tail.len() > 5 {
                    tail.remove(0);
                }
            }
        }
        tail
    });

    let is_video = format_info(&job.format).is_some_and(|f| f.video);
    let stdout = child.stdout.take().unwrap();
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let (mut index, mut total) = (1u32, 1u32);
    // Video downloads fetch two streams (picture, then sound); count them for honest progress.
    let mut stream = 0u32;
    for raw in BufReader::new(stdout).split(b'\n').map_while(Result::ok) {
        let line = String::from_utf8_lossy(&raw);
        let line = line.trim_end();
        if let Some(rest) = line.strip_prefix("SDJI ") {
            let Ok(v) = serde_json::from_str::<Value>(rest) else { continue };
            stream = 0;
            let title = v["title"].as_str().unwrap_or_default().to_string();
            let artist = v["uploader"].as_str().unwrap_or_default().trim_end_matches(" - Topic").to_string();
            let thumb = v["thumbnail"].as_str().map(|u| sources::small_artwork(job.site, u));
            index = v["playlist_index"].as_u64().unwrap_or(1) as u32;
            total = v["n_entries"].as_u64().or(v["playlist_count"].as_u64()).unwrap_or(1).max(1) as u32;
            let set_title = v["playlist_title"].as_str().map(str::to_string);
            update(&app, id, |j| {
                j.status = Status::Downloading;
                if j.kind == Kind::Track {
                    j.title = title;
                    j.artist = artist;
                    if thumb.is_some() {
                        j.artwork = thumb;
                    }
                } else {
                    j.current = Some(title);
                    j.index = Some(index);
                    j.total = Some(total);
                    if j.artist.is_empty() {
                        j.artist = artist;
                    }
                    if let Some(t) = set_title.filter(|_| j.title.is_empty() || j.title.starts_with("YouTube ")) {
                        j.title = t;
                    }
                    if j.artwork.is_none() {
                        j.artwork = thumb;
                    }
                    j.progress = (index - 1) as f64 / total as f64;
                }
            });
        } else if let Some(rest) = line.strip_prefix("SDJP ") {
            let Ok(v) = serde_json::from_str::<Value>(rest) else { continue };
            let finished = v["status"].as_str() == Some("finished");
            let frac = if finished {
                1.0
            } else if let (Some(d), Some(t)) = (num(&v, "downloaded_bytes"), num(&v, "total_bytes").or(num(&v, "total_bytes_estimate"))) {
                if t > 0.0 { d / t } else { 0.0 }
            } else if let (Some(i), Some(c)) = (num(&v, "fragment_index"), num(&v, "fragment_count")) {
                if c > 0.0 { i / c } else { 0.0 }
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            if !finished && last_emit.elapsed() < Duration::from_millis(120) {
                continue;
            }
            last_emit = Instant::now();
            // Video: the picture is most of the bytes, the sound the rest.
            let (item_frac, all_done) = match (is_video, stream) {
                (true, 0) => (frac * 0.85, false),
                (true, _) => (0.85 + frac * 0.15, finished),
                (false, _) => (frac, finished),
            };
            if finished {
                stream += 1;
            }
            let overall = ((index - 1) as f64 + item_frac) / total as f64;
            let (speed, eta) = (num(&v, "speed"), num(&v, "eta"));
            update(&app, id, |j| {
                j.status = if all_done { Status::Converting } else { Status::Downloading };
                j.progress = overall;
                j.speed = speed;
                j.eta = eta;
            });
        } else if let Some(rest) = line.strip_prefix("SDJF ") {
            let path = parse_printed_path(rest);
            update(&app, id, |j| {
                if !j.files.contains(&path) {
                    j.files.push(path);
                }
            });
        }
    }

    let status = child.wait();
    lock(&st.pids).remove(&id);
    let _ = std::fs::remove_dir_all(&temp);
    let errors = err_lines.join().unwrap_or_default();

    if lock(&st.cancelled).remove(&id) {
        update(&app, id, |j| {
            j.status = Status::Canceled;
            j.speed = None;
            j.eta = None;
        });
        pump(&app);
        return;
    }
    let ok = status.map(|s| s.success()).unwrap_or(false);
    let has_files = lock(&st.jobs).iter().any(|j| j.id == id && !j.files.is_empty());
    let result = if ok || (has_files && job.kind != Kind::Track) {
        Ok(if ok { None } else { Some("Some items were unavailable and were skipped.".to_string()) })
    } else {
        Err(friendly_error(errors.last().map(String::as_str).unwrap_or("")))
    };
    finish(&app, id, result);
}

fn finish(app: &AppHandle, id: u64, result: Result<Option<String>, String>) {
    let job = update(app, id, |j| {
        j.speed = None;
        j.eta = None;
        j.note = None;
        match result {
            Ok(warning) => {
                j.status = Status::Done;
                j.progress = 1.0;
                j.warning = warning;
                j.current = None;
            }
            Err(e) => {
                j.status = Status::Failed;
                j.error = Some(e);
            }
        }
    });
    save_history(app);
    if let Some(job) = job {
        notify_finished(app, &job);
    }
    pump(app);
}

/// A system notification when a download ends while the window is hidden or in the background.
fn notify_finished(app: &AppHandle, job: &Job) {
    use tauri_plugin_notification::NotificationExt;
    if !lock(&state(app).settings).notify {
        return;
    }
    let in_front = app
        .get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if in_front {
        return;
    }
    let name = [job.artist.as_str(), job.title.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" - ");
    let (title, body) = match job.status {
        Status::Done if job.kind != Kind::Track => ("Download finished", format!("{} ({} files)", job.title, job.files.len())),
        Status::Done => ("Download finished", name),
        _ => ("Download failed", format!("{name}: {}", job.error.clone().unwrap_or_default())),
    };
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Turn yt-dlp's "ERROR: [soundcloud] 123: Unable to …" into something a person can act on.
fn friendly_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("404") || lower.contains("not found") || lower.contains("video unavailable") || lower.contains("is unavailable") {
        "Couldn't find this. It may be private or deleted.".into()
    } else if lower.contains("country") || lower.contains("geo") {
        "This isn't available in your country.".into()
    } else if lower.contains("confirm your age") || lower.contains("age-restricted") || lower.contains("inappropriate") {
        "This is age-restricted on YouTube, so it can't be downloaded without a login.".into()
    } else if lower.contains("members-only") || lower.contains("join this channel") {
        "This is for channel members only.".into()
    } else if lower.contains("sign in") || lower.contains("login") || lower.contains("go+") || lower.contains("premium") {
        "This needs a login (for example SoundCloud Go+), so it can't be downloaded.".into()
    } else if lower.contains("live event") || lower.contains("is live") || lower.contains("premieres in") {
        "This live stream or premiere hasn't finished yet. Try again after it ends.".into()
    } else if lower.contains("timed out") || lower.contains("connection") || lower.contains("getaddrinfo") {
        "Network problem. Check your connection and retry.".into()
    } else if lower.contains("ffmpeg") || lower.contains("postprocessing") {
        "Converting failed. Try “Original audio”, or repair the engine in Settings.".into()
    } else if raw.is_empty() {
        "Download failed. Retry, or update the engine in Settings.".into()
    } else {
        // Strip "ERROR: [extractor] id: " noise.
        let msg = raw.trim_start_matches("ERROR:").trim();
        let msg = match (msg.starts_with('['), msg.find(']')) {
            (true, Some(i)) => msg[i + 1..].trim_start_matches([':', ' ']),
            _ => msg,
        };
        let msg = match msg.split_once(": ") {
            Some((id, rest)) if !id.contains(' ') => rest,
            _ => msg,
        };
        msg.chars().take(160).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(site: Site, format: &str) -> Job {
        Job {
            id: 1, url: "https://example.com/x".into(), site, kind: Kind::Track, format: format.into(),
            title: String::new(), artist: String::new(), artwork: None, status: Status::Queued, progress: 0.0,
            speed: None, eta: None, current: None, index: None, total: None, files: vec![], error: None,
            warning: None, note: None, created: 0, source: "app".into(),
        }
    }

    fn args(j: &Job) -> String {
        let ctx = RunCtx { out_dir: "out", temp_dir: "tmp", ffmpeg: "ffmpeg", embed_art: true, js: Some(("node".into(), PathBuf::from("node"))) };
        build_args(j, &ctx).join(" ")
    }

    #[test]
    fn slug_to_title() {
        assert_eq!(pretty_slug("https://soundcloud.com/forss/flickermood"), "Flickermood");
        assert_eq!(pretty_slug("https://soundcloud.com/a/sets/late-night-mix"), "Late Night Mix");
    }

    #[test]
    fn keeps_non_latin_file_names() {
        // Exactly what yt-dlp prints for "Ask The Rhymes سل القوافي".
        let printed = r#""C:\\Users\\me\\Music\\slothDJ\\Supra_pyx - Ask The Rhymes \u0633\u0644 \u0627\u0644\u0642\u0648\u0627\u0641\u064a.mp3""#;
        assert_eq!(
            parse_printed_path(printed),
            r"C:\Users\me\Music\slothDJ\Supra_pyx - Ask The Rhymes سل القوافي.mp3"
        );
        assert_eq!(parse_printed_path("/music/a b.mp3 "), "/music/a b.mp3");
    }

    #[test]
    fn asks_ytdlp_for_utf8_and_json_paths() {
        let a = args(&job(Site::SoundCloud, "mp3_320"));
        assert!(a.contains("--encoding utf-8"));
        assert!(a.contains("after_move:SDJF %(filepath)j"));
        assert!(!a.contains("--js-runtimes"), "SoundCloud doesn't need a JS runtime");
    }

    #[test]
    fn youtube_audio_and_video_args() {
        let a = args(&job(Site::YouTube, "mp3_192"));
        assert!(a.contains("--js-runtimes node:node"));
        assert!(a.contains("--audio-quality 192K"));
        assert!(a.contains("ThumbnailsConvertor"), "square cover art for YouTube audio");

        let v = args(&job(Site::YouTube, "mp4_720"));
        assert!(v.contains("-S res:720,vcodec:h264,acodec:m4a"));
        assert!(v.contains("--merge-output-format mp4"));
        assert!(!v.contains("--audio-format"));
    }

    #[test]
    fn video_formats_only_for_youtube() {
        assert_eq!(effective_format(Some("mp4_720"), Site::YouTube, "mp3_320"), "mp4_720");
        assert_eq!(effective_format(Some("mp4_720"), Site::SoundCloud, "mp3_192"), "mp3_192");
        assert_eq!(effective_format(Some("mp4_720"), Site::SoundCloud, "mp4_best"), "mp3_320");
        assert_eq!(effective_format(None, Site::SoundCloud, "original"), "original");
        assert_eq!(effective_format(Some("nope"), Site::YouTube, "mp3_128"), "mp3_128");
    }

    #[test]
    fn errors_are_friendly() {
        assert!(friendly_error("ERROR: [soundcloud] 123: HTTP Error 404: Not Found").contains("private or deleted"));
        assert!(friendly_error("ERROR: [youtube] abc: Sign in to confirm your age").contains("age-restricted"));
        assert_eq!(friendly_error("ERROR: [soundcloud] 123: Something odd happened"), "Something odd happened");
    }
}
