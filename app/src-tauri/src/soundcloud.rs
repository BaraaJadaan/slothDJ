//! SoundCloud links: which ones we accept, what they point at, and a fast preview
//! through SoundCloud's public oEmbed API.

use crate::sources::{oembed, Kind, OembedError, Preview, Site, Target};
use crate::util::http;

/// First path segments that are SoundCloud pages, not users.
const RESERVED: &[&str] = &[
    "discover", "stream", "search", "upload", "you", "settings", "messages",
    "notifications", "charts", "pages", "terms-of-use", "imprint", "mobile", "jobs",
    "creators", "popular", "feed", "people", "tags", "connect", "logout", "signin",
    "signup", "pro", "artists", "go", "home", "for-artists",
];

/// Second segments that turn a profile URL into a list of sounds.
const PROFILE_TABS: &[&str] = &[
    "tracks", "albums", "sets", "reposts", "likes", "popular-tracks", "spotlight", "toptracks",
];

/// Third segments that are just sub-pages of a track.
const TRACK_SUBPAGES: &[&str] = &["recommended", "likes", "reposts", "comments", "sets", "albums"];

pub fn is_host(host: &str) -> bool {
    host == "soundcloud.com" || host == "on.soundcloud.com"
}

fn target(url: String, kind: Kind) -> Target {
    Target { url, kind, site: Site::SoundCloud }
}

pub fn parse(url: &url::Url, host: &str) -> Result<Target, String> {
    if host == "on.soundcloud.com" {
        // Short share link: resolved to a full URL just before downloading.
        return Ok(target(format!("https://on.soundcloud.com{}", url.path()), Kind::Track));
    }
    let mut segs: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    // SoundCloud's newer pages live under /n/ (/n/artist/track): the same sound as /artist/track.
    if segs.len() > 1 && segs[0] == "n" {
        segs.remove(0);
    }
    let Some(first) = segs.first().map(|s| s.to_ascii_lowercase()) else {
        return Err("Paste a link to a track, set or profile.".into());
    };
    let is_system_set = first == "discover" && segs.len() >= 3 && segs[1] == "sets";
    if RESERVED.contains(&first.as_str()) && !is_system_set {
        return Err("That page can't be downloaded. Open a track, set or profile.".into());
    }

    let (path, kind) = match segs.len() {
        // A bare profile means "this artist's own tracks" (not their reposts).
        1 => (format!("/{}/tracks", segs[0]), Kind::Profile),
        2 if PROFILE_TABS.contains(&segs[1]) => (format!("/{}/{}", segs[0], segs[1]), Kind::Profile),
        2 => (format!("/{}/{}", segs[0], segs[1]), Kind::Track),
        _ if segs[1] == "sets" => (format!("/{}", segs[..segs.len().min(4)].join("/")), Kind::Set),
        _ if TRACK_SUBPAGES.contains(&segs[2]) => (format!("/{}/{}", segs[0], segs[1]), Kind::Track),
        // /user/track/s-SECRET (private share link)
        _ => (format!("/{}", segs[..3].join("/")), Kind::Track),
    };
    Ok(target(format!("https://soundcloud.com{path}"), kind))
}

/// Follow an on.soundcloud.com short link to the real page.
pub fn resolve(t: Target) -> Result<Target, String> {
    if !t.url.starts_with("https://on.soundcloud.com") {
        return Ok(t);
    }
    let resp = http()
        .get(&t.url)
        .call()
        .map_err(|_| "Couldn't open that short link.".to_string())?;
    crate::sources::parse(resp.get_url())
}

/// Title, artist and artwork in ~200 ms, without starting the download engine.
pub fn preview(t: &Target) -> Result<Preview, String> {
    let t = resolve(t.clone())?;
    let v = match oembed("https://soundcloud.com/oembed", &t.url) {
        Ok(v) => v,
        Err(OembedError::Status(404 | 403)) => {
            return Err("Couldn't find that on SoundCloud. It may be private or removed.".into())
        }
        Err(_) => return Err("Couldn't reach SoundCloud. Check your connection.".into()),
    };
    let artist = v["author_name"].as_str().unwrap_or_default().to_string();
    let mut title = v["title"].as_str().unwrap_or_default().to_string();
    if let Some(stripped) = title.strip_suffix(&format!(" by {artist}")) {
        title = stripped.to_string();
    }
    let artwork = v["thumbnail_url"].as_str().map(small_artwork);
    Ok(Preview { url: t.url, kind: t.kind, site: Site::SoundCloud, title, artist, artwork })
}

/// SoundCloud artwork URLs carry their size in the name; ask for a light one.
pub fn small_artwork(url: &str) -> String {
    let url = url.replacen("http://", "https://", 1);
    for size in ["-original.", "-t500x500.", "-large.", "-crop."] {
        if url.contains(size) {
            return url.replacen(size, "-t300x300.", 1);
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::parse;

    #[test]
    fn classifies_links() {
        let t = parse("https://soundcloud.com/forss/flickermood?in=x/sets/y&si=abc").unwrap();
        assert_eq!(t.kind, Kind::Track);
        assert_eq!(t.url, "https://soundcloud.com/forss/flickermood");

        let t = parse("soundcloud.com/forss/sets/soulhack").unwrap();
        assert_eq!(t.kind, Kind::Set);

        // The newer SoundCloud pages put /n/ in front.
        let t = parse("https://soundcloud.com/n/forss/flickermood").unwrap();
        assert_eq!(t.kind, Kind::Track);
        assert_eq!(t.url, "https://soundcloud.com/forss/flickermood");

        let t = parse("https://m.soundcloud.com/forss").unwrap();
        assert_eq!((t.kind, t.url.as_str()), (Kind::Profile, "https://soundcloud.com/forss/tracks"));

        let t = parse("https://soundcloud.com/forss/likes").unwrap();
        assert_eq!(t.kind, Kind::Profile);

        let t = parse("https://soundcloud.com/forss/flickermood/s-AbCd").unwrap();
        assert_eq!((t.kind, t.url.as_str()), (Kind::Track, "https://soundcloud.com/forss/flickermood/s-AbCd"));

        let t = parse("https://soundcloud.com/forss/flickermood/recommended").unwrap();
        assert_eq!(t.url, "https://soundcloud.com/forss/flickermood");

        assert!(parse("https://soundcloud.com/discover").is_err());
    }

    #[test]
    fn shrinks_artwork() {
        assert_eq!(
            small_artwork("http://i1.sndcdn.com/artworks-1-t500x500.jpg"),
            "https://i1.sndcdn.com/artworks-1-t300x300.jpg"
        );
    }
}
