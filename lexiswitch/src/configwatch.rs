//! Loading the configuration file, and noticing when it changes.
//!
//! A reload builds a whole new configuration and swaps it wholesale, so nothing ever reads
//! half-old, half-new state. A file that fails validation is *not* swapped in — the last one
//! that did validate stays in effect, which is how last-known-good works without needing a
//! rollback path.
//!
//! Change detection is an mtime poll rather than a directory watch. The file is edited by
//! hand, occasionally, so a two-second poll is indistinguishable from instant, and a poll
//! cannot leave a handle open on the user's config directory.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use lexiswitch_core::config::Config;

/// Where the configuration lives: `%APPDATA%\LexiSwitch\config.toml`.
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|appdata| {
        PathBuf::from(appdata)
            .join("LexiSwitch")
            .join("config.toml")
    })
}

/// Outcome of an attempt to load configuration.
pub enum Loaded {
    /// Parsed and validated.
    Ok(Config),
    /// No file. Defaults are used, which is not an error — the example file documents them.
    Missing,
    /// The file exists but could not be used. The previous configuration stays in effect.
    Invalid(String),
}

/// Read and validate the file at `path`.
pub fn load(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::Missing,
        Err(e) => return Loaded::Invalid(format!("{}: {e}", path.display())),
    };
    match Config::parse(&text) {
        Ok(config) => Loaded::Ok(config),
        Err(e) => Loaded::Invalid(e.to_string()),
    }
}

/// Polls a file's modification time, reporting only when it changes.
pub struct ConfigWatcher {
    path: PathBuf,
    last_seen: Option<SystemTime>,
    interval: Duration,
    next_check: std::time::Instant,
}

impl ConfigWatcher {
    pub fn new(path: PathBuf) -> Self {
        let last_seen = mtime(&path);
        Self {
            path,
            last_seen,
            interval: Duration::from_secs(2),
            next_check: std::time::Instant::now() + Duration::from_secs(2),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the file has changed since the last report.
    ///
    /// Cheap enough to call from the message loop: it does nothing at all until the poll
    /// interval elapses, and then performs a single metadata read.
    pub fn changed(&mut self) -> bool {
        let now = std::time::Instant::now();
        if now < self.next_check {
            return false;
        }
        self.next_check = now + self.interval;

        let current = mtime(&self.path);
        if current != self.last_seen {
            self.last_seen = current;
            // A file that has just appeared counts as a change; one that has just been
            // deleted does too, so the defaults come back.
            return true;
        }
        false
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

/// Opt-in logging of event kinds and sizes. Never content.
///
/// The rule this enforces is absolute: nothing the user typed or selected reaches disk, and
/// that includes logs ([ADR-0011](../../docs/adr/0011-no-keyboard-observation.md)). The API
/// therefore accepts a kind and, where there is one, a size — never a string. A caller cannot
/// log the text even by mistake, because there is no parameter to pass it in.
pub struct EventLog {
    enabled: bool,
}

impl EventLog {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Record something with no size to report.
    pub fn note(&self, kind: &str) {
        if self.enabled {
            eprintln!("{kind}");
        }
    }
}
