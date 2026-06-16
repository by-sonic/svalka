use crate::state::{now_unix, AppState};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use svalka_core::leftovers::{find_leftovers, Leftover};
use svalka_core::model::{Category, CategoryId};
use svalka_core::programs::{list_programs, Program};
use svalka_core::quarantine::QuarantineEntry;
use svalka_core::scanner::scan as core_scan;
use svalka_core::uninstaller::{uninstall_queue, UninstallResult};
use tauri::State;

/// Disambiguates quarantine ids created within the same second.
static SEQ: AtomicU64 = AtomicU64::new(0);

#[tauri::command]
pub fn scan(categories: Vec<CategoryId>) -> Vec<Category> {
    core_scan(&categories)
}

#[tauri::command]
pub fn get_programs() -> Vec<Program> {
    list_programs()
}

#[derive(Serialize)]
pub struct QuarantineOutcome {
    pub ok: bool,
    pub message: String,
    pub freed: u64,
}

#[tauri::command]
pub fn quarantine_items(
    state: State<AppState>,
    source: String,
    volume_root: PathBuf,
    paths: Vec<PathBuf>,
) -> QuarantineOutcome {
    let id = format!("q{}-{}", now_unix(), SEQ.fetch_add(1, Ordering::Relaxed));
    let mut q = state.quarantine.lock().unwrap();
    match q.quarantine_paths(&id, &source, now_unix(), &volume_root, &paths, false) {
        Ok(entry) => QuarantineOutcome {
            ok: true,
            message: "в карантине".into(),
            freed: entry.total(),
        },
        Err(e) => QuarantineOutcome { ok: false, message: e.to_string(), freed: 0 },
    }
}

#[tauri::command]
pub fn list_quarantine(state: State<AppState>) -> Vec<QuarantineEntry> {
    state.quarantine.lock().unwrap().list().to_vec()
}

#[tauri::command]
pub fn restore_entry(state: State<AppState>, id: String) -> Result<(), String> {
    state
        .quarantine
        .lock()
        .unwrap()
        .restore(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn uninstall_programs(programs: Vec<Program>) -> Vec<UninstallResult> {
    uninstall_queue(&programs, |_| {})
}

#[tauri::command]
pub fn find_leftovers_cmd(program: Program) -> Vec<Leftover> {
    find_leftovers(&program)
}
