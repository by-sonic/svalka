use std::path::PathBuf;
use std::sync::Mutex;
use svalka_core::quarantine::QuarantineStore;

pub struct AppState {
    pub quarantine: Mutex<QuarantineStore>,
}

impl AppState {
    pub fn new() -> Self {
        let meta = meta_dir();
        let mut store = QuarantineStore::new(meta);
        // Auto-purge quarantine entries older than 30 days on launch.
        store.purge_older_than(30, now_unix());
        Self { quarantine: Mutex::new(store) }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn meta_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("Svalka")
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
