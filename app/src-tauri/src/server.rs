//! Tiny local API the browser extension talks to.
//!
//! Listens on 127.0.0.1 only. Requests from web pages are refused: browsers always
//! send an `Origin` header for cross-site requests, and only extension origins
//! (or local tools that send none) are accepted. The `Host` check stops DNS rebinding.

use crate::jobs::{self, FORMATS};
use crate::state::{lock, state};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use tauri::{AppHandle, Emitter};
use tiny_http::{Header, Method, Request, Response, Server};

pub const PORTS: [u16; 5] = [37771, 37772, 37773, 37774, 37775];

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let Some((server, port)) = PORTS
            .iter()
            .find_map(|&p| Server::http(("127.0.0.1", p)).ok().map(|s| (s, p)))
        else {
            eprintln!("SlothDJ: no free port for the extension API");
            return;
        };
        *lock(&state(&app).port) = Some(port);
        for req in server.incoming_requests() {
            handle(&app, port, req);
        }
    });
}

fn header(req: &Request, name: &'static str) -> Option<String> {
    req.headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str().to_string())
}

fn origin_allowed(origin: Option<&str>) -> bool {
    match origin {
        None => true,
        Some(o) => ["chrome-extension://", "moz-extension://", "safari-web-extension://"]
            .iter()
            .any(|p| o.starts_with(p)),
    }
}

/// Remember that the browser extension is installed (and its version, sent as `?ext=1.2.0`),
/// so the app stops suggesting you install it.
fn note_extension(app: &AppHandle, query: &str) {
    let version = url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == "ext")
        .map(|(_, v)| v.chars().filter(|c| c.is_ascii_digit() || *c == '.').take(20).collect::<String>())
        .filter(|v| !v.is_empty());
    let st = state(app);
    let now = crate::util::now_ms();
    let changed = {
        let mut s = lock(&st.settings);
        let stale = now.saturating_sub(s.extension_seen) > 60_000;
        let new_version = version.is_some() && version != s.extension_version;
        if stale || new_version {
            s.extension_seen = now;
            if version.is_some() {
                s.extension_version = version;
            }
        }
        stale || new_version
    };
    if changed {
        st.save_settings();
        let s = lock(&st.settings).clone();
        let _ = app.emit("settings", &s);
    }
}

#[derive(Deserialize)]
struct DownloadBody {
    url: String,
    format: Option<String>,
}

fn handle(app: &AppHandle, port: u16, mut req: Request) {
    let origin = header(&req, "Origin");
    let host = header(&req, "Host").unwrap_or_default();
    let host_ok = host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}");

    let mut cors = vec![];
    if let Some(o) = origin.as_deref().filter(|o| origin_allowed(Some(o))) {
        cors.push(Header::from_bytes("Access-Control-Allow-Origin", o).unwrap());
        cors.push(Header::from_bytes("Access-Control-Allow-Headers", "content-type").unwrap());
        cors.push(Header::from_bytes("Access-Control-Allow-Methods", "GET, POST, OPTIONS").unwrap());
        cors.push(Header::from_bytes("Access-Control-Allow-Private-Network", "true").unwrap());
    }
    let reply = |req: Request, code: u16, body: Value| {
        let mut resp = Response::from_string(body.to_string())
            .with_status_code(code)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        for h in cors.iter().cloned() {
            resp.add_header(h);
        }
        let _ = req.respond(resp);
    };

    if !host_ok || !origin_allowed(origin.as_deref()) {
        return reply(req, 403, json!({ "ok": false, "error": "forbidden" }));
    }

    let method = req.method().clone();
    let (path, query) = match req.url().split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (req.url().to_string(), String::new()),
    };
    let st = state(app);
    // Browsers don't send an Origin header on an extension's plain GET requests, so the
    // `?ext=` tag the extension adds counts too. Web pages were already refused above.
    let from_extension = origin.as_deref().is_some_and(|o| o.contains("-extension://"));
    if from_extension || query.split('&').any(|kv| kv.starts_with("ext=")) {
        note_extension(app, &query);
    }

    match (method, path.as_str()) {
        (Method::Options, _) => reply(req, 204, json!({})),

        (Method::Get, "/api/status") => {
            let (ready, version) = {
                let e = lock(&st.engine);
                (e.info.ready, e.info.ytdlp_version.clone())
            };
            reply(req, 200, json!({
                "app": "slothDJ",
                "version": env!("CARGO_PKG_VERSION"),
                "ready": ready,
                "engine": version,
                "defaultFormat": lock(&st.settings).format,
                "formats": FORMATS,
                "sites": ["soundcloud", "youtube"],
            }))
        }

        (Method::Post, "/api/download") => {
            let is_json = header(&req, "Content-Type").is_some_and(|c| c.starts_with("application/json"));
            let mut body = String::new();
            let _ = req.as_reader().take(16 * 1024).read_to_string(&mut body);
            let parsed = serde_json::from_str::<DownloadBody>(&body);
            match (is_json, parsed) {
                (true, Ok(b)) => match jobs::enqueue(app, &b.url, b.format.as_deref(), "extension") {
                    Ok(job) => {
                        let _ = app.emit("toast", format!("Added from your browser: {}", job.title));
                        reply(req, 200, json!({ "ok": true, "job": job }))
                    }
                    Err(e) => reply(req, 400, json!({ "ok": false, "error": e })),
                },
                _ => reply(req, 400, json!({ "ok": false, "error": "Send JSON: {\"url\": \"…\"}" })),
            }
        }

        (Method::Post, p) if p.starts_with("/api/jobs/") && p.ends_with("/cancel") => {
            let id: u64 = p.trim_start_matches("/api/jobs/").trim_end_matches("/cancel").parse().unwrap_or(0);
            jobs::cancel(app, id);
            reply(req, 200, json!({ "ok": true }))
        }

        (Method::Get, p) if p.starts_with("/api/jobs/") => {
            let id: u64 = p.trim_start_matches("/api/jobs/").parse().unwrap_or(0);
            let job = lock(&st.jobs).iter().find(|j| j.id == id).cloned();
            match job {
                Some(j) => reply(req, 200, json!({ "ok": true, "job": j })),
                None => reply(req, 404, json!({ "ok": false, "error": "not found" })),
            }
        }

        (Method::Post, "/api/show") => {
            crate::show_main(app);
            reply(req, 200, json!({ "ok": true }))
        }

        _ => reply(req, 404, json!({ "ok": false, "error": "not found" })),
    }
}

#[cfg(test)]
mod tests {
    use super::origin_allowed;

    #[test]
    fn only_extensions_and_local_tools() {
        assert!(origin_allowed(None));
        assert!(origin_allowed(Some("chrome-extension://abcdef")));
        assert!(origin_allowed(Some("moz-extension://1234")));
        assert!(!origin_allowed(Some("https://evil.example")));
        assert!(!origin_allowed(Some("null")));
    }
}
