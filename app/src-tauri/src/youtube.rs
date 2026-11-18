//! YouTube links: videos (incl. Shorts and YouTube Music) and playlists, plus a fast
//! preview through YouTube's public oEmbed API.

use crate::sources::{oembed, Kind, OembedError, Preview, Site, Target};

pub fn is_host(host: &str) -> bool {
    matches!(host, "youtube.com" | "music.youtube.com" | "youtu.be" | "youtube-nocookie.com")
}

fn valid_video_id(id: &str) -> bool {
    id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn valid_list_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn video(id: &str) -> Result<Target, String> {
    if !valid_video_id(id) {
        return Err("That YouTube link looks incomplete. Copy it again from the video.".into());
    }
    Ok(Target { url: format!("https://www.youtube.com/watch?v={id}"), kind: Kind::Track, site: Site::YouTube })
}

fn playlist(id: &str) -> Result<Target, String> {
    // Watch Later, Liked videos and Liked music only exist behind your login.
    if matches!(id, "WL" | "LL" | "LM") {
        return Err("That's a private list that needs your YouTube login.".into());
    }
    if !valid_list_id(id) {
        return Err("That YouTube playlist link looks incomplete.".into());
    }
    Ok(Target { url: format!("https://www.youtube.com/playlist?list={id}"), kind: Kind::Set, site: Site::YouTube })
}

pub fn parse(url: &url::Url, host: &str) -> Result<Target, String> {
    let segs: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    let query = |key: &str| url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned());

    if host == "youtu.be" {
        return video(segs.first().copied().unwrap_or_default());
    }
    match segs.first().copied() {
        // A video opened from a playlist downloads just that video.
        Some("watch") => video(&query("v").unwrap_or_default()),
        Some("shorts" | "live" | "embed" | "v") => video(segs.get(1).copied().unwrap_or_default()),
        Some("playlist") => playlist(&query("list").unwrap_or_default()),
        Some(s) if s.starts_with('@') || matches!(s, "channel" | "c" | "user") => {
            Err("YouTube channels aren't supported yet. Open a video or a playlist.".into())
        }
        _ => Err("Open a YouTube video or playlist, then copy its link.".into()),
    }
}

fn video_id(url: &str) -> Option<&str> {
    url.split_once("watch?v=").map(|(_, id)| &id[..id.len().min(11)])
}

pub fn preview(t: &Target) -> Result<Preview, String> {
    let fallback_art = video_id(&t.url).map(|id| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"));
    let v = match oembed("https://www.youtube.com/oembed", &t.url) {
        Ok(v) => v,
        // 401/403: the owner disabled embedding. The video can still be downloaded.
        Err(OembedError::Status(401 | 403)) => {
            let title = if t.kind == Kind::Set { "YouTube playlist" } else { "YouTube video" };
            return Ok(Preview { url: t.url.clone(), kind: t.kind, site: Site::YouTube, title: title.into(), artist: String::new(), artwork: fallback_art });
        }
        Err(OembedError::Status(400 | 404)) => {
            return Err("Couldn't find that on YouTube. It may be private or removed.".into())
        }
        Err(_) => return Err("Couldn't reach YouTube. Check your connection.".into()),
    };
    let artist = v["author_name"].as_str().unwrap_or_default().trim_end_matches(" - Topic").to_string();
    let title = v["title"].as_str().unwrap_or_default().to_string();
    let artwork = fallback_art.or_else(|| v["thumbnail_url"].as_str().map(small_artwork));
    Ok(Preview { url: t.url.clone(), kind: t.kind, site: Site::YouTube, title, artist, artwork })
}

/// `…/vi/ID/maxresdefault.webp` → the small 320×180 JPEG of the same video.
pub fn small_artwork(url: &str) -> String {
    match url.split_once("/vi/").or_else(|| url.split_once("/vi_webp/")) {
        Some((_, rest)) => {
            let id = rest.split('/').next().unwrap_or_default();
            format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")
        }
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::parse;

    #[test]
    fn classifies_links() {
        let want = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
        for link in [
            "https://www.youtube.com/watch?v=jNQXAC9IVRw&list=PL123&t=5s",
            "youtu.be/jNQXAC9IVRw?si=abc",
            "https://m.youtube.com/watch?v=jNQXAC9IVRw",
            "https://music.youtube.com/watch?v=jNQXAC9IVRw&feature=share",
            "https://www.youtube.com/shorts/jNQXAC9IVRw",
            "https://www.youtube.com/embed/jNQXAC9IVRw",
        ] {
            let t = parse(link).unwrap();
            assert_eq!((t.url.as_str(), t.kind, t.site), (want, Kind::Track, Site::YouTube), "{link}");
        }

        let t = parse("https://www.youtube.com/playlist?list=PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG").unwrap();
        assert_eq!(t.kind, Kind::Set);
        assert_eq!(t.url, "https://www.youtube.com/playlist?list=PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG");

        assert!(parse("https://www.youtube.com/@forss").unwrap_err().contains("channels"));
        assert!(parse("https://www.youtube.com/playlist?list=WL").unwrap_err().contains("login"));
        assert!(parse("https://www.youtube.com/watch?v=short").is_err());
        assert!(parse("https://www.youtube.com/feed/trending").is_err());
    }

    #[test]
    fn shrinks_artwork() {
        assert_eq!(
            small_artwork("https://i.ytimg.com/vi_webp/jNQXAC9IVRw/maxresdefault.webp"),
            "https://i.ytimg.com/vi/jNQXAC9IVRw/mqdefault.jpg"
        );
    }
}
