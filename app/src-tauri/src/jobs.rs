//! The download queue: one job per pasted link, run by yt-dlp with live progress.

use crate::soundcloud::{self, Kind};
use crate::state::{lock, state};
use crate::util::{command, now_ms, write_json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const HISTORY_LIMIT: usize = 300;

/// Output formats offered in the app and the extension.
pub const FORMATS: &[(&str, &str)] = &[
    ("mp3_320", "MP3 · 320 kbps"),
    ("mp3_192", "MP3 · 192 kbps"),
    ("mp3_128", "MP3 · 128 kbps"),
    ("original", "Original (no re-encode)"),
];

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
    pub kind: Kind,
    pub format: String,
    pub title: String,
    pub artist: String,
    pub artwork: Option<String>,
    pub status: Status,
    pub progress: f64,
    pub speed: Option<f64>,
    pub eta: Option<f64>,
    /// For sets/profiles: the track currently downloading.
    pub current: Option<String>,
    pub index: Option<u32>,
    pub total: Option<u32>,
    pub files: Vec<String>,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub created: u64,
    pub source: String,
}

fn emit(app: &AppHandle, job: &Job) {
    let _ = app.emit("job", job);
}

fn update(app: &AppHandle, id: u64, f: impl FnOnce(&mut Job)) -> Option<Job> {
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
    let target = soundcloud::parse(url)?;
    let st = state(app);
    let format = format
        .filter(|f| FORMATS.iter().any(|(k, _)| k == f))
        .map(str::to_string)
        .unwrap_or_else(|| lock(&st.settings).format.clone());

    // Same link already queued or running? Don't download it twice.
    if let Some(existing) = lock(&st.jobs).iter().find(|j| j.url == target.url && !j.status.is_finished()) {
        return Ok(existing.clone());
    }

    let job = Job {
        id: st.next_id.fetch_add(1, Ordering::Relaxed),
        url: target.url.clone(),
        kind: target.kind,
        format,
        title: pretty_slug(&target.url),
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
        created: now_ms(),
        source: source.into(),
    };
    lock(&st.jobs).push(job.clone());
    emit(app, &job);
    save_history(app);

    // Fill in the real title/artist/artwork quickly via oEmbed.
    let (app2, id) = (app.clone(), job.id);
    std::thread::spawn(move || {
        if let Ok(p) = soundcloud::preview(&target) {
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
}

pub fn cancel(app: &AppHandle, id: u64) {
    let st = state(app);
    let status = lock(&st.jobs).iter().find(|j| j.id == id).map(|j| j.status);
    match status {
        Some(Status::Queued) => {
            update(app, id, |j| j.status = Status::Canceled);
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

fn build_args(job: &Job, out_dir: &str, temp_dir: &str, ffmpeg: &str, embed_art: bool) -> Vec<String> {
    let mut a: Vec<String> = [
        "--ignore-config", "--no-simulate", "--newline", "--progress", "--color", "never",
        "--no-mtime", "--concurrent-fragments", "4", "--retries", "5", "--fragment-retries", "10",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut push = |items: &[&str]| a.extend(items.iter().map(|s| s.to_string()));
    push(&["--ffmpeg-location", ffmpeg]);
    push(&["-P", &format!("home:{out_dir}"), "-P", &format!("temp:{temp_dir}")]);

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
    push(&["--print", "after_move:SDJF %(filepath)s"]);
    push(&[
        "--progress-template",
        "download:SDJP %(progress.{status,downloaded_bytes,total_bytes,total_bytes_estimate,speed,eta,fragment_index,fragment_count})j",
    ]);

    push(&["--embed-metadata"]);
    if embed_art {
        push(&["--embed-thumbnail", "--convert-thumbnails", "jpg"]);
    }
    match job.format.as_str() {
        "original" => push(&["-f", "bestaudio/best", "-x"]),
        f => {
            let kbps = f.strip_prefix("mp3_").unwrap_or("320");
            push(&["-f", "bestaudio/best", "-x", "--audio-format", "mp3", "--audio-quality", &format!("{kbps}K")]);
        }
    }
    push(&["--", &job.url]);
    a
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn run(app: AppHandle, id: u64) {
    let st = state(&app);
    let Some(mut job) = lock(&st.jobs).iter().find(|j| j.id == id).cloned() else { return };

    // Short links need resolving before yt-dlp sees them.
    if job.url.starts_with("https://on.soundcloud.com") {
        match soundcloud::parse(&job.url).and_then(soundcloud::resolve) {
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

    let settings = lock(&st.settings).clone();
    let (ytdlp, ffmpeg) = {
        let e = lock(&st.engine);
        (e.ytdlp.clone(), e.ffmpeg.clone().unwrap_or_default())
    };
    let _ = std::fs::create_dir_all(&settings.output_dir);
    let temp = st.paths.temp_dir.join(id.to_string());
    let _ = std::fs::create_dir_all(&temp);
    let args = build_args(
        &job,
        &settings.output_dir,
        &temp.to_string_lossy(),
        &ffmpeg.to_string_lossy(),
        settings.embed_artwork,
    );

    let child = command(&ytdlp)
        .args(&args)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
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
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if line.starts_with("ERROR:") {
                tail.push(line);
                if tail.len() > 5 {
                    tail.remove(0);
                }
            }
        }
        tail
    });

    let stdout = child.stdout.take().unwrap();
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let (mut index, mut total) = (1u32, 1u32);
    for raw in BufReader::new(stdout).split(b'\n').map_while(Result::ok) {
        let line = String::from_utf8_lossy(&raw);
        let line = line.trim_end();
        if let Some(rest) = line.strip_prefix("SDJI ") {
            let Ok(v) = serde_json::from_str::<Value>(rest) else { continue };
            let title = v["title"].as_str().unwrap_or_default().to_string();
            let artist = v["uploader"].as_str().unwrap_or_default().to_string();
            let thumb = v["thumbnail"].as_str().map(soundcloud::small_artwork);
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
                    if let Some(t) = set_title.filter(|_| j.title.is_empty()) {
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
            let overall = ((index - 1) as f64 + frac) / total as f64;
            let (speed, eta) = (num(&v, "speed"), num(&v, "eta"));
            update(&app, id, |j| {
                j.status = if finished { Status::Converting } else { Status::Downloading };
                j.progress = overall;
                j.speed = speed;
                j.eta = eta;
            });
        } else if let Some(path) = line.strip_prefix("SDJF ") {
            let path = path.to_string();
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
        Ok(if ok { None } else { Some("Some tracks were unavailable and were skipped.".to_string()) })
    } else {
        Err(friendly_error(errors.last().map(String::as_str).unwrap_or("")))
    };
    finish(&app, id, result);
}

fn finish(app: &AppHandle, id: u64, result: Result<Option<String>, String>) {
    update(app, id, |j| {
        j.speed = None;
        j.eta = None;
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
    pump(app);
}

/// Turn yt-dlp's "ERROR: [soundcloud] 123: Unable to …" into something a person can act on.
fn friendly_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("404") || lower.contains("not found") {
        "Couldn't find this on SoundCloud. It may be private or deleted.".into()
    } else if lower.contains("country") || lower.contains("geo") {
        "This isn't available in your country.".into()
    } else if lower.contains("sign in") || lower.contains("login") || lower.contains("go+") || lower.contains("premium") {
        "This needs a SoundCloud Go+ login, so it can't be downloaded.".into()
    } else if lower.contains("timed out") || lower.contains("connection") || lower.contains("getaddrinfo") {
        "Network problem. Check your connection and retry.".into()
    } else if lower.contains("ffmpeg") || lower.contains("postprocessing") {
        "Converting failed. Try “Original” quality, or repair the engine in Settings.".into()
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

    #[test]
    fn slug_to_title() {
        assert_eq!(pretty_slug("https://soundcloud.com/forss/flickermood"), "Flickermood");
        assert_eq!(pretty_slug("https://soundcloud.com/a/sets/late-night-mix"), "Late Night Mix");
    }

    #[test]
    fn errors_are_friendly() {
        assert!(friendly_error("ERROR: [soundcloud] 123: HTTP Error 404: Not Found").contains("private or deleted"));
        assert_eq!(friendly_error("ERROR: [soundcloud] 123: Something odd happened"), "Something odd happened");
    }
}
