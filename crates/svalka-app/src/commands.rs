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
use svalka_core::usage::{analyze, Limits, UsageReport};
use tauri::{async_runtime::spawn_blocking, Emitter, State};

/// Disambiguates quarantine ids created within the same second.
static SEQ: AtomicU64 = AtomicU64::new(0);

// Every command here offloads its real work to `spawn_blocking`. Tauri runs a
// synchronous command on the main thread, so the heavy filesystem/registry work
// these do (sizing trees, walking hives, moving files) would freeze the whole
// UI — which is exactly what made the Programs screen hang. Running async on a
// blocking thread keeps the window responsive.

#[tauri::command]
pub async fn scan(window: tauri::Window, categories: Vec<CategoryId>) -> Vec<Category> {
    spawn_blocking(move || {
        let mut out = Vec::new();
        for id in &categories {
            let cats = core_scan(std::slice::from_ref(id));
            for c in &cats {
                // Now that scanning is off the main thread, these progress
                // events actually render as they arrive.
                let _ = window.emit("scan://progress", &c.label);
            }
            out.extend(cats);
        }
        out
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn get_programs() -> Vec<Program> {
    spawn_blocking(list_programs).await.unwrap_or_default()
}

#[derive(Serialize)]
pub struct SkippedOut {
    pub path: String,
    pub reason: String,
}

#[derive(Serialize)]
pub struct QuarantineOutcome {
    pub ok: bool,
    pub message: String,
    pub freed: u64,
    pub moved: usize,
    pub skipped: Vec<SkippedOut>,
}

#[tauri::command]
pub async fn quarantine_items(
    state: State<'_, AppState>,
    source: String,
    volume_root: PathBuf,
    paths: Vec<PathBuf>,
) -> Result<QuarantineOutcome, String> {
    // Clone the handle before awaiting: a `State` borrow can't cross `.await`.
    let store = state.quarantine.clone();
    let id = format!("q{}-{}", now_unix(), SEQ.fetch_add(1, Ordering::Relaxed));

    spawn_blocking(move || {
        let mut q = store.lock().unwrap();
        match q.quarantine_paths(&id, &source, now_unix(), &volume_root, &paths, false) {
            Ok(report) => {
                let moved = report.entry.files.len();
                let freed = report.entry.total();
                let skipped: Vec<SkippedOut> = report
                    .skipped
                    .iter()
                    .map(|s| SkippedOut {
                        path: s.path.display().to_string(),
                        reason: s.reason.clone(),
                    })
                    .collect();
                let message = match (moved, skipped.len()) {
                    (0, 0) => "нечего переносить".into(),
                    (_, 0) => "в карантине".into(),
                    (0, n) => format!("не удалось перенести {n} — заняты или недоступны"),
                    (m, n) => format!("в карантине {m}, пропущено {n} (заняты или недоступны)"),
                };
                QuarantineOutcome {
                    ok: moved > 0,
                    message,
                    freed,
                    moved,
                    skipped,
                }
            }
            Err(e) => QuarantineOutcome {
                ok: false,
                message: e.to_string(),
                freed: 0,
                moved: 0,
                skipped: Vec::new(),
            },
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_quarantine(state: State<'_, AppState>) -> Result<Vec<QuarantineEntry>, String> {
    // Async so it never blocks the main thread waiting on the lock while a
    // quarantine is mid-flight on a blocking thread.
    let store = state.quarantine.clone();
    spawn_blocking(move || store.lock().unwrap().list().to_vec())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_entry(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let store = state.quarantine.clone();
    spawn_blocking(move || store.lock().unwrap().restore(&id).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_entry(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let store = state.quarantine.clone();
    spawn_blocking(move || store.lock().unwrap().delete(&id).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn clear_quarantine(state: State<'_, AppState>) -> Result<u64, String> {
    let store = state.quarantine.clone();
    spawn_blocking(move || store.lock().unwrap().clear().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn uninstall_programs(programs: Vec<Program>) -> Vec<UninstallResult> {
    spawn_blocking(move || uninstall_queue(&programs, |_| {}))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn find_leftovers_cmd(program: Program) -> Vec<Leftover> {
    spawn_blocking(move || find_leftovers(&program))
        .await
        .unwrap_or_default()
}

/// Open an http(s) link in the user's default browser (e.g. the donation page).
/// Scheme is validated so this can never be coerced into running something
/// else, and `explorer.exe` receives the URL as a single argument (no shell
/// parsing) and launches the browser de-elevated.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("разрешены только http(s) ссылки".into());
    }
    std::process::Command::new("explorer")
        .arg(&url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Analyze disk usage under `root`. Heavy walk runs on a blocking thread (a
/// whole-drive walk on the main thread would freeze the UI — same lesson as the
/// Programs screen). Emits `analyze://progress` per top-level folder.
#[tauri::command]
pub async fn analyze_disk(window: tauri::Window, root: PathBuf) -> Result<UsageReport, String> {
    spawn_blocking(move || {
        analyze(&root, Limits::default(), |name| {
            let _ = window.emit("analyze://progress", name);
        })
    })
    .await
    .map_err(|e| e.to_string())
}

/// Existing logical drive roots (e.g. `["C:\\", "D:\\"]`) for the picker.
#[tauri::command]
pub async fn list_drives() -> Vec<String> {
    spawn_blocking(|| {
        (b'A'..=b'Z')
            .filter_map(|c| {
                let root = format!("{}:\\", c as char);
                if std::path::Path::new(&root).exists() {
                    Some(root)
                } else {
                    None
                }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}
