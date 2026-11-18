//! "Type a song name" search across SoundCloud and YouTube, using yt-dlp's built-in
//! search (scsearch / ytsearch). Both sites are queried at the same time.

use crate::sources::{self, Kind, Site};
use crate::state::{lock, state};
use crate::util::command;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use tauri::AppHandle;

const PER_SITE: usize = 8;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub site: Site,
    pub url: String,
    pub title: String,
    pub artist: String,
    pub duration: Option<f64>,
    pub artwork: Option<String>,
}

pub fn search(app: &AppHandle, query: &str) -> Result<Vec<Hit>, String> {
    let query = query.trim();
    if query.chars().count() < 2 {
        return Err("Type at least two letters to search.".into());
    }
    let st = state(app);
    let ytdlp = {
        let e = lock(&st.engine);
        if !e.info.ready {
            return Err("The download engine is still starting. Try again in a moment.".into());
        }
        e.ytdlp.clone()
    };

    let spawn = |prefix: &str, site: Site| {
        let (ytdlp, q) = (ytdlp.clone(), format!("{prefix}{PER_SITE}:{query}"));
        std::thread::spawn(move || one_site(&ytdlp, &q, site))
    };
    let youtube = spawn("ytsearch", Site::YouTube);
    let soundcloud = spawn("scsearch", Site::SoundCloud);
    let failed = || Err("Search stopped unexpectedly.".to_string());
    let (youtube, soundcloud) = (youtube.join().unwrap_or_else(|_| failed()), soundcloud.join().unwrap_or_else(|_| failed()));

    match (youtube, soundcloud) {
        (Err(e), Err(_)) => Err(e),
        (yt, sc) => Ok(yt.unwrap_or_default().into_iter().chain(sc.unwrap_or_default()).collect()),
    }
}

fn one_site(ytdlp: &Path, query: &str, site: Site) -> Result<Vec<Hit>, String> {
    let out = command(ytdlp)
        .args(["--ignore-config", "--no-warnings", "--encoding", "utf-8", "--flat-playlist", "-J", "--", query])
        .output()
        .map_err(|e| format!("Search failed: {e}"))?;
    let v: Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| "Search failed. Check your connection and try again.".to_string())?;
    Ok(parse_entries(&v, site))
}

fn parse_entries(v: &Value, site: Site) -> Vec<Hit> {
    v["entries"]
        .as_array()
        .map(|entries| entries.iter().filter_map(|e| hit(e, site)).collect())
        .unwrap_or_default()
}

fn hit(e: &Value, site: Site) -> Option<Hit> {
    // Skip live streams and premieres: there's nothing finished to download yet.
    if matches!(e["live_status"].as_str(), Some("is_live" | "is_upcoming")) {
        return None;
    }
    let link = e["webpage_url"].as_str().or(e["url"].as_str())?;
    let target = sources::parse(link).ok().filter(|t| t.kind == Kind::Track && t.site == site)?;
    let artwork = match site {
        Site::YouTube => e["id"].as_str().map(|id| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")),
        Site::SoundCloud => e["thumbnail"]
            .as_str()
            .or_else(|| e["thumbnails"].as_array()?.last()?["url"].as_str())
            .map(|u| sources::small_artwork(site, u)),
    };
    Some(Hit {
        site,
        url: target.url,
        title: e["title"].as_str()?.to_string(),
        artist: e["uploader"]
            .as_str()
            .or(e["channel"].as_str())
            .unwrap_or_default()
            .trim_end_matches(" - Topic")
            .to_string(),
        duration: e["duration"].as_f64(),
        artwork,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_search_results() {
        let yt = json!({ "entries": [
            { "id": "1idKkQnE_rY", "url": "https://www.youtube.com/watch?v=1idKkQnE_rY", "title": "Flickermood", "channel": "Forss - Topic", "duration": 214.0 },
            { "id": "liveliveliv", "url": "https://www.youtube.com/watch?v=liveliveliv", "title": "Radio", "live_status": "is_live" },
            { "url": "https://www.youtube.com/@forss", "title": "A channel" }
        ]});
        let hits = parse_entries(&yt, Site::YouTube);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].artist, "Forss");
        assert_eq!(hits[0].artwork.as_deref(), Some("https://i.ytimg.com/vi/1idKkQnE_rY/mqdefault.jpg"));

        let sc = json!({ "entries": [
            { "url": "https://api.soundcloud.com/tracks/293", "webpage_url": "https://soundcloud.com/forss/flickermood",
              "title": "Flickermood", "uploader": "Forss", "duration": 213.9,
              "thumbnails": [{ "url": "https://i1.sndcdn.com/artworks-000067273316-smsiqx-original.jpg" }] }
        ]});
        let hits = parse_entries(&sc, Site::SoundCloud);
        assert_eq!(hits[0].url, "https://soundcloud.com/forss/flickermood");
        assert_eq!(hits[0].artwork.as_deref(), Some("https://i1.sndcdn.com/artworks-000067273316-smsiqx-t300x300.jpg"));
    }
}
