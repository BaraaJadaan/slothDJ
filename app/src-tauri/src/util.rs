//! Small helpers shared by the other modules.

use serde::{de::DeserializeOwned, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A `Command` that never flashes a console window on Windows.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        // Own process group, so cancelling also stops the ffmpeg child.
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    c
}

/// Kill a process and everything it spawned.
pub fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = command("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status();
    }
}

pub fn exe_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

/// Find an executable on PATH.
pub fn which(name: &str) -> Option<PathBuf> {
    let exe = exe_name(name);
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(&exe))
        .find(|f| f.is_file())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write JSON atomically (temp file + rename) so a crash never leaves half a file.
pub fn write_json<T: Serialize>(path: &Path, value: &T) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    if let Ok(text) = serde_json::to_string_pretty(value) {
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, path);
        }
    }
}

pub fn http() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .user_agent(concat!("slothDJ/", env!("CARGO_PKG_VERSION")))
        .build()
}
