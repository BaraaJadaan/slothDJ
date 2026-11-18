//! The sites slothDJ can download from. Each site module turns a pasted link into a
//! canonical `Target` and a quick `Preview`; everything else (queue, engine) is shared.

use crate::{soundcloud, youtube};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Site {
    #[default]
    SoundCloud,
    YouTube,
}

/// What a link points at. YouTube uses Track for a video and Set for a playlist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Track,
    Set,
    Profile,
}

#[derive(Clone, Debug)]
pub struct Target {
    pub url: String,
    pub kind: Kind,
    pub site: Site,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub url: String,
    pub kind: Kind,
    pub site: Site,
    pub title: String,
    pub artist: String,
    pub artwork: Option<String>,
}

/// Validate and normalise a link the user pasted (or the extension sent).
pub fn parse(input: &str) -> Result<Target, String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err("Paste a SoundCloud or YouTube link first.".into());
    }
    let with_scheme = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let url = url::Url::parse(&with_scheme).map_err(|_| "That doesn't look like a link.".to_string())?;
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    let host = host
        .strip_prefix("www.")
        .or_else(|| host.strip_prefix("m."))
        .unwrap_or(&host)
        .to_string();

    if youtube::is_host(&host) {
        youtube::parse(&url, &host)
    } else if soundcloud::is_host(&host) {
        soundcloud::parse(&url, &host)
    } else {
        Err("slothDJ supports SoundCloud and YouTube links.".into())
    }
}

/// Follow short links (on.soundcloud.com) to the real page.
pub fn resolve(target: Target) -> Result<Target, String> {
    match target.site {
        Site::SoundCloud => soundcloud::resolve(target),
        Site::YouTube => Ok(target),
    }
}

pub fn preview(target: &Target) -> Result<Preview, String> {
    match target.site {
        Site::SoundCloud => soundcloud::preview(target),
        Site::YouTube => youtube::preview(target),
    }
}

/// Artwork URLs from yt-dlp are often huge; ask each site for a light one.
pub fn small_artwork(site: Site, url: &str) -> String {
    match site {
        Site::SoundCloud => soundcloud::small_artwork(url),
        Site::YouTube => youtube::small_artwork(url),
    }
}

pub(crate) enum OembedError {
    /// The site answered with this HTTP status.
    Status(u16),
    /// Couldn't reach the site or read its answer.
    Network,
}

/// Shared oEmbed lookup: both sites expose title, author and thumbnail this way.
pub(crate) fn oembed(endpoint: &str, page: &str) -> Result<serde_json::Value, OembedError> {
    let api = url::Url::parse_with_params(endpoint, &[("format", "json"), ("url", page)])
        .expect("static oEmbed endpoint");
    match crate::util::http().get(api.as_str()).call() {
        Ok(resp) => resp.into_json().map_err(|_| OembedError::Network),
        Err(ureq::Error::Status(code, _)) => Err(OembedError::Status(code)),
        Err(_) => Err(OembedError::Network),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_by_site() {
        assert_eq!(parse("https://soundcloud.com/forss/flickermood").unwrap().site, Site::SoundCloud);
        assert_eq!(parse("youtu.be/jNQXAC9IVRw").unwrap().site, Site::YouTube);
        assert!(parse("https://example.com/x").unwrap_err().contains("SoundCloud and YouTube"));
        assert!(parse("   ").is_err());
    }
}
