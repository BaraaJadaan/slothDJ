//! Preview: something playable for a link, so you can listen before downloading.
//!
//! SoundCloud has an official embeddable player. YouTube often blocks embedding
//! (especially music videos), so for YouTube we ask yt-dlp for a small 360p stream
//! that the app's web view can play directly.

use crate::sources::{self, Kind, Site};
use crate::state::{lock, state};
use crate::util::command;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stream {
    /// "video" (play `src` in a <video>) or "widget" (show `src` in an iframe).
    pub kind: &'static str,
    pub src: String,
}

pub fn soundcloud_widget(track_url: &str) -> String {
    url::Url::parse_with_params(
        "https://w.soundcloud.com/player/",
        &[
            ("url", track_url),
            ("auto_play", "true"),
            ("visual", "true"),
            ("hide_related", "true"),
            ("show_comments", "false"),
            ("show_reposts", "false"),
            ("show_teaser", "false"),
        ],
    )
    .expect("static widget URL")
    .into()
}

pub fn preview_stream(app: &AppHandle, link: &str) -> Result<Stream, String> {
    let target = sources::resolve(sources::parse(link)?)?;
    match target.site {
        Site::SoundCloud => Ok(Stream { kind: "widget", src: soundcloud_widget(&target.url) }),
        Site::YouTube => {
            if target.kind != Kind::Track {
                return Err("Previews work for single videos. Open one video from the playlist.".into());
            }
            let ytdlp = {
                let st = state(app);
                let e = lock(&st.engine);
                if !e.info.ready {
                    return Err("The download engine is still starting. Try again in a moment.".into());
                }
                e.ytdlp.clone()
            };
            let js = crate::engine::js_runtime(app, None)?;
            let mut cmd = command(&ytdlp);
            cmd.args(["--ignore-config", "--no-warnings", "--encoding", "utf-8", "--no-playlist"]);
            if let Some((name, path)) = js {
                cmd.args(["--no-js-runtimes", "--js-runtimes", &format!("{name}:{}", path.to_string_lossy())]);
            }
            // Format 18 is YouTube's 360p MP4 with sound in one file: quick to start, plays anywhere.
            cmd.args([
                "-f",
                "18/b[height<=480][vcodec!=none][acodec!=none][ext=mp4]/b[vcodec!=none][acodec!=none]",
                "--print",
                "url",
                "--",
                &target.url,
            ]);
            let out = cmd.output().map_err(|e| format!("Couldn't start the engine: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            match stdout.lines().find(|l| l.starts_with("https://")) {
                Some(src) => Ok(Stream { kind: "video", src: src.to_string() }),
                None => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let last = stderr.lines().rfind(|l| l.starts_with("ERROR:")).unwrap_or("");
                    Err(crate::jobs::friendly_error(last))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soundcloud_widget_url() {
        let w = soundcloud_widget("https://soundcloud.com/forss/flickermood");
        assert!(w.starts_with("https://w.soundcloud.com/player/?url=https%3A%2F%2Fsoundcloud.com%2Fforss%2Fflickermood"));
        assert!(w.contains("auto_play=true"));
    }
}
