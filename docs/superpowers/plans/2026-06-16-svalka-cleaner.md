# Svalka Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Svalka — a safe, transparent Windows cleaner: scans real junk (system / browser / dev caches), uninstalls programs in batches with real folder sizes, hunts uninstall leftovers, and makes every deletion reversible via a quarantine zone.

**Architecture:** Rust core crate (`svalka-core`) holds all logic and is tested headless. A Tauri wrapper (`svalka-app`) maps IPC commands/events onto the core. A Svelte webview renders four screens (Очистка / Программы / Карантин / Настройки). Deletion = atomic move into a per-volume quarantine; registry keys are exported to `.reg` first.

**Tech Stack:** Rust (stable), Tauri 2, Svelte + Vite, crates: `winreg`, `windows`, `jwalk`, `rayon`, `serde`/`serde_json`, `tempfile`, `serial_test`.

**Milestones (each produces working, tested software):**
- **M1 — Core engine (Phases 0–7):** `cargo test -p svalka-core` green. A headless library + tiny CLI smoke-test that scans, sizes, quarantines, restores. This alone is a portfolio-worthy, fully-tested artifact.
- **M2 — App + UI (Phases 8–10):** Tauri app with the four screens wired to the core.

**Reference:** spec at `docs/superpowers/specs/2026-06-16-svalka-design.md`.

---

## File Structure

```
Svalka/
  Cargo.toml                         # workspace
  crates/
    svalka-core/
      Cargo.toml
      src/
        lib.rs                       # re-exports modules
        model.rs                     # CleanItem, ItemKind, Category, CategoryId
        fsafe.rs                     # atomic move, same-volume check, in-use detection, dry-run
        quarantine.rs                # QuarantineStore: quarantine/restore/purge + manifest
        regbackup.rs                 # export/import registry keys via reg.exe
        sizer.rs                     # parallel directory sizing (jwalk)
        scanner.rs                   # junk categories + allow/deny guard
        programs.rs                  # installed programs from registry + UWP, real sizes
        uninstaller.rs               # uninstall-string parsing + queue
        leftovers.rs                 # orphan folder/registry matching
        paths.rs                     # env-resolved well-known roots (testable via injection)
      tests/
        quarantine_roundtrip.rs
    svalka-app/
      Cargo.toml
      tauri.conf.json                # requireAdministrator manifest
      build.rs
      src/
        main.rs                      # Tauri builder, command registration
        commands.rs                  # thin IPC wrappers over svalka-core
        state.rs                     # shared AppState (Quarantine, settings)
  ui/
    package.json
    vite.config.ts
    src/
      ipc.ts                         # typed wrappers over @tauri-apps/api invoke/listen
      App.svelte                     # sidebar nav shell
      lib/format.ts                  # human-readable byte formatting
      routes/
        Clean.svelte                 # Очистка
        Programs.svelte              # Программы
        Quarantine.svelte            # Карантин
        Settings.svelte              # Настройки
  docs/superpowers/{specs,plans}/
```

**Decomposition rationale:** one module = one responsibility. `fsafe` and `quarantine` are the safety foundation and are built/tested first; everything destructive routes through them. The core never imports Tauri, so it compiles and tests on any host.

---

# Phase 0 — Workspace scaffolding

### Task 0.1: Create the Cargo workspace and core crate

**Files:**
- Create: `Cargo.toml`
- Create: `crates/svalka-core/Cargo.toml`
- Create: `crates/svalka-core/src/lib.rs`

- [ ] **Step 1: Create workspace manifest**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/svalka-core", "crates/svalka-app"]

[workspace.package]
edition = "2021"
version = "0.1.0"
```

- [ ] **Step 2: Create core crate manifest**

`crates/svalka-core/Cargo.toml`:
```toml
[package]
name = "svalka-core"
edition.workspace = true
version.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
jwalk = "0.8"
rayon = "1"

[target.'cfg(windows)'.dependencies]
winreg = "0.52"

[dev-dependencies]
tempfile = "3"
serial_test = "3"
```

- [ ] **Step 3: Create lib.rs with module stubs**

`crates/svalka-core/src/lib.rs`:
```rust
pub mod model;
pub mod paths;
pub mod fsafe;
pub mod sizer;
pub mod regbackup;
pub mod quarantine;
pub mod scanner;
pub mod programs;
pub mod uninstaller;
pub mod leftovers;
```

Create empty files for each module so the crate compiles:
`model.rs`, `paths.rs`, `fsafe.rs`, `sizer.rs`, `regbackup.rs`, `quarantine.rs`, `scanner.rs`, `programs.rs`, `uninstaller.rs`, `leftovers.rs` — each containing a single line `// implemented in later tasks`.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p svalka-core`
Expected: compiles with warnings about empty modules, no errors.

- [ ] **Step 5: Commit**

```bash
git init
git add .
git commit -m "chore: scaffold cargo workspace and svalka-core crate"
```

---

# Phase 1 — Shared model

### Task 1.1: Define core data types

**Files:**
- Modify: `crates/svalka-core/src/model.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/svalka-core/src/model.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn category_total_sums_item_sizes() {
        let cat = Category {
            id: CategoryId::SystemTemp,
            label: "Временные файлы".into(),
            items: vec![
                CleanItem { kind: ItemKind::Dir, path: PathBuf::from("a"), size: 100 },
                CleanItem { kind: ItemKind::File, path: PathBuf::from("b"), size: 50 },
            ],
        };
        assert_eq!(cat.total(), 150);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core model::`
Expected: FAIL — `Category`, `CleanItem`, etc. not found.

- [ ] **Step 3: Write the types**

Replace the top of `crates/svalka-core/src/model.rs` (above the test module):
```rust
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ItemKind {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanItem {
    pub kind: ItemKind,
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CategoryId {
    SystemTemp,
    RecycleBin,
    ThumbnailCache,
    CrashDumps,
    Wer,
    WindowsUpdate,
    BrowserCache,
    DevCache,
}

#[derive(Debug, Clone, Serialize)]
pub struct Category {
    pub id: CategoryId,
    pub label: String,
    pub items: Vec<CleanItem>,
}

impl Category {
    pub fn total(&self) -> u64 {
        self.items.iter().map(|i| i.size).sum()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core model::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/model.rs
git commit -m "feat(core): add CleanItem/Category/CategoryId model"
```

---

# Phase 2 — fsafe (deletion foundation)

### Task 2.1: Same-volume detection

**Files:**
- Modify: `crates/svalka-core/src/fsafe.rs`

- [ ] **Step 1: Write the failing test**

`crates/svalka-core/src/fsafe.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn same_volume_true_for_same_drive() {
        assert!(same_volume(Path::new(r"C:\a\b"), Path::new(r"C:\c\d")));
    }

    #[test]
    fn same_volume_false_for_different_drives() {
        assert!(!same_volume(Path::new(r"C:\a"), Path::new(r"D:\b")));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core fsafe::`
Expected: FAIL — `same_volume` not found.

- [ ] **Step 3: Implement same_volume**

Prepend to `crates/svalka-core/src/fsafe.rs`:
```rust
use std::path::{Component, Path, PathBuf};

fn volume_prefix(p: &Path) -> Option<String> {
    for c in p.components() {
        if let Component::Prefix(pre) = c {
            return Some(pre.as_os_str().to_string_lossy().to_uppercase());
        }
    }
    None
}

pub fn same_volume(a: &Path, b: &Path) -> bool {
    match (volume_prefix(a), volume_prefix(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core fsafe::`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/fsafe.rs
git commit -m "feat(core): same-volume detection in fsafe"
```

### Task 2.2: move_to with dry-run

**Files:**
- Modify: `crates/svalka-core/src/fsafe.rs`

- [ ] **Step 1: Write the failing test**

Add inside the `tests` module in `fsafe.rs`:
```rust
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn move_to_relocates_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("junk.tmp");
        fs::write(&src, b"data").unwrap();
        let dst_dir = dir.path().join("q");
        fs::create_dir_all(&dst_dir).unwrap();

        let moved = move_to(&src, &dst_dir, false).unwrap();

        assert!(!src.exists());
        assert!(moved.exists());
        assert_eq!(fs::read(&moved).unwrap(), b"data");
    }

    #[test]
    fn move_to_dry_run_keeps_source() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("junk.tmp");
        fs::write(&src, b"data").unwrap();
        let dst_dir = dir.path().join("q");
        fs::create_dir_all(&dst_dir).unwrap();

        let planned = move_to(&src, &dst_dir, true).unwrap();

        assert!(src.exists());
        assert!(!planned.exists());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core fsafe::`
Expected: FAIL — `move_to` not found.

- [ ] **Step 3: Implement move_to and FsError**

Add to `fsafe.rs` (above tests):
```rust
#[derive(Debug)]
pub enum FsError {
    InUse(PathBuf),
    CrossVolume(PathBuf, PathBuf),
    Io(std::io::Error),
}

impl std::fmt::Display for FsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FsError::InUse(p) => write!(f, "файл занят: {}", p.display()),
            FsError::CrossVolume(a, b) => {
                write!(f, "разные тома: {} -> {}", a.display(), b.display())
            }
            FsError::Io(e) => write!(f, "ошибка ввода-вывода: {e}"),
        }
    }
}

impl From<std::io::Error> for FsError {
    fn from(e: std::io::Error) -> Self {
        FsError::Io(e)
    }
}

/// Move `src` into `dst_dir`, returning the new path. Uses an atomic rename
/// (instant, no extra space) which requires the same volume. When `dry_run`
/// is true, returns the planned destination without touching the filesystem.
pub fn move_to(src: &Path, dst_dir: &Path, dry_run: bool) -> Result<PathBuf, FsError> {
    let name = src
        .file_name()
        .ok_or_else(|| FsError::Io(std::io::Error::other("источник без имени")))?;
    let dst = dst_dir.join(name);

    if dry_run {
        return Ok(dst);
    }
    if !same_volume(src, dst_dir) {
        return Err(FsError::CrossVolume(src.to_path_buf(), dst_dir.to_path_buf()));
    }
    match std::fs::rename(src, &dst) {
        Ok(()) => Ok(dst),
        Err(e) if e.raw_os_error() == Some(32) => Err(FsError::InUse(src.to_path_buf())),
        Err(e) => Err(FsError::Io(e)),
    }
}
```

> Note: in `tempdir()` tests both paths share the temp volume, so `same_volume` passes. OS error 32 is `ERROR_SHARING_VIOLATION` (file in use).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core fsafe::`
Expected: PASS (all fsafe tests).

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/fsafe.rs
git commit -m "feat(core): move_to with dry-run and in-use detection"
```

---

# Phase 3 — sizer

### Task 3.1: Parallel directory sizing

**Files:**
- Modify: `crates/svalka-core/src/sizer.rs`

- [ ] **Step 1: Write the failing test**

`crates/svalka-core/src/sizer.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn dir_size_sums_nested_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.bin"), vec![0u8; 1000]).unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("b.bin"), vec![0u8; 2000]).unwrap();

        assert_eq!(dir_size(dir.path()), 3000);
    }

    #[test]
    fn dir_size_zero_for_missing() {
        assert_eq!(dir_size(std::path::Path::new(r"Z:\nope-svalka")), 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core sizer::`
Expected: FAIL — `dir_size` not found.

- [ ] **Step 3: Implement dir_size with jwalk**

Prepend to `sizer.rs`:
```rust
use jwalk::WalkDir;
use std::path::Path;

/// Total size of a directory tree in bytes. Parallel walk via jwalk's rayon
/// pool. Missing paths and unreadable entries contribute 0 (never panics).
pub fn dir_size(root: &Path) -> u64 {
    if !root.exists() {
        return 0;
    }
    if root.is_file() {
        return std::fs::metadata(root).map(|m| m.len()).unwrap_or(0);
    }
    WalkDir::new(root)
        .skip_hidden(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core sizer::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/sizer.rs
git commit -m "feat(core): parallel directory sizing via jwalk"
```

---

# Phase 4 — quarantine + regbackup (safety core)

### Task 4.1: Quarantine manifest types

**Files:**
- Modify: `crates/svalka-core/src/quarantine.rs`

- [ ] **Step 1: Write the failing test**

`crates/svalka-core/src/quarantine.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_total_sums_moved_files() {
        let entry = QuarantineEntry {
            id: "e1".into(),
            created: 0,
            source: "Очистка".into(),
            files: vec![
                MovedFile { original: "a".into(), stored: "q/a".into(), size: 10 },
                MovedFile { original: "b".into(), stored: "q/b".into(), size: 20 },
            ],
            reg: vec![],
        };
        assert_eq!(entry.total(), 30);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core quarantine::`
Expected: FAIL — types not found.

- [ ] **Step 3: Implement the manifest types**

Prepend to `quarantine.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovedFile {
    pub original: PathBuf,
    pub stored: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegBackup {
    pub hive: String,
    pub subkey: String,
    pub reg_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineEntry {
    pub id: String,
    pub created: u64,
    pub source: String,
    pub files: Vec<MovedFile>,
    pub reg: Vec<RegBackup>,
}

impl QuarantineEntry {
    pub fn total(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core quarantine::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/quarantine.rs
git commit -m "feat(core): quarantine manifest types"
```

### Task 4.2: QuarantineStore — quarantine + restore (files), round-trip

**Files:**
- Modify: `crates/svalka-core/src/quarantine.rs`
- Test: `crates/svalka-core/tests/quarantine_roundtrip.rs`

- [ ] **Step 1: Write the failing integration test**

`crates/svalka-core/tests/quarantine_roundtrip.rs`:
```rust
use svalka_core::quarantine::QuarantineStore;
use std::fs;
use tempfile::tempdir;

#[test]
fn file_roundtrip_restores_bytes_to_original_path() {
    let vol = tempdir().unwrap();        // pretend "volume" root
    let meta = tempdir().unwrap();       // %LocalAppData%\Svalka

    // a junk file we will quarantine then restore
    let junk = vol.path().join("project").join("cache.bin");
    fs::create_dir_all(junk.parent().unwrap()).unwrap();
    fs::write(&junk, b"precious-cache").unwrap();

    let mut store = QuarantineStore::new(meta.path().to_path_buf());
    let entry = store
        .quarantine_paths("test", "Очистка", 1000, vol.path(), &[junk.clone()], false)
        .unwrap();

    assert!(!junk.exists(), "source should be moved out");
    assert_eq!(entry.total(), 14);

    store.restore("test").unwrap();

    assert!(junk.exists(), "file should be back");
    assert_eq!(fs::read(&junk).unwrap(), b"precious-cache");
    assert!(store.list().is_empty(), "entry removed after restore");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core --test quarantine_roundtrip`
Expected: FAIL — `QuarantineStore` not found.

- [ ] **Step 3: Implement QuarantineStore**

Add to `quarantine.rs` (above tests). Note the per-volume store dir keeps moves same-volume:
```rust
use crate::fsafe::{move_to, FsError};
use std::path::Path;

const STORE_DIRNAME: &str = ".svalka-quarantine";

pub struct QuarantineStore {
    meta_dir: PathBuf,
    entries: Vec<QuarantineEntry>,
}

impl QuarantineStore {
    /// `meta_dir` is the central metadata dir (e.g. %LocalAppData%\Svalka).
    pub fn new(meta_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&meta_dir);
        let entries = Self::load(&meta_dir);
        Self { meta_dir, entries }
    }

    fn manifest_path(meta: &Path) -> PathBuf {
        meta.join("quarantine.json")
    }

    fn load(meta: &Path) -> Vec<QuarantineEntry> {
        std::fs::read(Self::manifest_path(meta))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_vec_pretty(&self.entries) {
            let _ = std::fs::write(Self::manifest_path(&self.meta_dir), json);
        }
    }

    pub fn list(&self) -> &[QuarantineEntry] {
        &self.entries
    }

    /// Move `paths` into `volume_root\.svalka-quarantine\<id>` (same-volume,
    /// instant). `id`/`now` are injected so callers (and tests) stay
    /// deterministic. On `dry_run`, computes the entry without moving.
    pub fn quarantine_paths(
        &mut self,
        id: &str,
        source: &str,
        now: u64,
        volume_root: &Path,
        paths: &[PathBuf],
        dry_run: bool,
    ) -> Result<QuarantineEntry, FsError> {
        let store_dir = volume_root.join(STORE_DIRNAME).join(id);
        if !dry_run {
            std::fs::create_dir_all(&store_dir)?;
        }
        let mut files = Vec::new();
        for p in paths {
            let size = crate::sizer::dir_size(p);
            let stored = move_to(p, &store_dir, dry_run)?;
            files.push(MovedFile { original: p.clone(), stored, size });
        }
        let entry = QuarantineEntry {
            id: id.to_string(),
            created: now,
            source: source.to_string(),
            files,
            reg: Vec::new(),
        };
        if !dry_run {
            self.entries.push(entry.clone());
            self.save();
        }
        Ok(entry)
    }

    /// Move every file in the entry back to its original path, re-import any
    /// registry backups, then drop the entry.
    pub fn restore(&mut self, entry_id: &str) -> Result<(), FsError> {
        let idx = self
            .entries
            .iter()
            .position(|e| e.id == entry_id)
            .ok_or_else(|| FsError::Io(std::io::Error::other("запись карантина не найдена")))?;
        let entry = self.entries[idx].clone();
        for f in &entry.files {
            if let Some(parent) = f.original.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&f.stored, &f.original)?;
        }
        for r in &entry.reg {
            crate::regbackup::import_reg(&r.reg_file)
                .map_err(|e| FsError::Io(std::io::Error::other(e)))?;
        }
        self.entries.remove(idx);
        self.save();
        Ok(())
    }
}
```

> `regbackup::import_reg` is implemented in Task 4.4; the `reg` vec is empty in this test so it is not exercised yet.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core --test quarantine_roundtrip`
Expected: FAIL to compile — `regbackup::import_reg` missing. Add a temporary stub in `regbackup.rs`:
```rust
pub fn import_reg(_reg_file: &std::path::Path) -> Result<(), String> {
    Ok(())
}
```
Re-run: `cargo test -p svalka-core --test quarantine_roundtrip`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/quarantine.rs crates/svalka-core/src/regbackup.rs crates/svalka-core/tests/quarantine_roundtrip.rs
git commit -m "feat(core): QuarantineStore file quarantine + restore round-trip"
```

### Task 4.3: Auto-purge old entries

**Files:**
- Modify: `crates/svalka-core/src/quarantine.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `quarantine.rs`:
```rust
    use tempfile::tempdir;

    #[test]
    fn purge_removes_entries_older_than_cutoff() {
        let meta = tempdir().unwrap();
        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        store.push_for_test(QuarantineEntry {
            id: "old".into(), created: 0, source: "x".into(), files: vec![], reg: vec![],
        });
        store.push_for_test(QuarantineEntry {
            id: "new".into(), created: 100_000, source: "x".into(), files: vec![], reg: vec![],
        });

        let day = 86_400u64;
        let removed = store.purge_older_than(1, 1 * day + 50_000);

        assert_eq!(removed, 1);
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].id, "new");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core quarantine::`
Expected: FAIL — `push_for_test` / `purge_older_than` not found.

- [ ] **Step 3: Implement purge + test helper**

Add to the `impl QuarantineStore` block:
```rust
    /// Delete entries whose `created` is older than `days` before `now`
    /// (both in unix seconds). Removes stored files and returns the count.
    pub fn purge_older_than(&mut self, days: u64, now: u64) -> u64 {
        let cutoff = now.saturating_sub(days.saturating_mul(86_400));
        let mut removed = 0u64;
        self.entries.retain(|e| {
            if e.created < cutoff {
                for f in &e.files {
                    let _ = std::fs::remove_dir_all(&f.stored);
                    let _ = std::fs::remove_file(&f.stored);
                }
                removed += 1;
                false
            } else {
                true
            }
        });
        if removed > 0 {
            self.save();
        }
        removed
    }

    #[cfg(test)]
    pub fn push_for_test(&mut self, e: QuarantineEntry) {
        self.entries.push(e);
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core quarantine::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/quarantine.rs
git commit -m "feat(core): quarantine auto-purge by age"
```

### Task 4.4: Registry backup via reg.exe (Windows)

**Files:**
- Modify: `crates/svalka-core/src/regbackup.rs`

- [ ] **Step 1: Replace the stub with a real implementation + test**

`crates/svalka-core/src/regbackup.rs` (replace the stub entirely):
```rust
use std::path::Path;
use std::process::Command;

/// Export a registry key (e.g. `HKCU\Software\Foo`) to a `.reg` file.
pub fn export_reg(full_key: &str, out_file: &Path) -> Result<(), String> {
    run_reg(&["export", full_key, &out_file.to_string_lossy(), "/y"])
}

/// Re-import a previously exported `.reg` file.
pub fn import_reg(reg_file: &Path) -> Result<(), String> {
    run_reg(&["import", &reg_file.to_string_lossy()])
}

fn run_reg(args: &[&str]) -> Result<(), String> {
    let out = Command::new("reg")
        .args(args)
        .output()
        .map_err(|e| format!("не удалось запустить reg.exe: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;
    use winreg::enums::*;
    use winreg::RegKey;

    #[test]
    #[serial]
    fn export_then_delete_then_import_restores_value() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let subkey = r"Software\SvalkaTest\Backup";
        let (key, _) = hkcu.create_subkey(subkey).unwrap();
        key.set_value("Marker", &"hello").unwrap();

        let dir = tempdir().unwrap();
        let reg_file = dir.path().join("backup.reg");
        export_reg(&format!(r"HKCU\{subkey}"), &reg_file).unwrap();

        hkcu.delete_subkey_all(r"Software\SvalkaTest").unwrap();
        assert!(hkcu.open_subkey(subkey).is_err());

        import_reg(&reg_file).unwrap();
        let restored = hkcu.open_subkey(subkey).unwrap();
        let v: String = restored.get_value("Marker").unwrap();
        assert_eq!(v, "hello");

        hkcu.delete_subkey_all(r"Software\SvalkaTest").unwrap();
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p svalka-core regbackup:: -- --nocapture`
Expected: PASS on Windows (uses a scratch key under `HKCU\Software\SvalkaTest`, cleaned up after).

- [ ] **Step 3: Commit**

```bash
git add crates/svalka-core/src/regbackup.rs
git commit -m "feat(core): registry export/import via reg.exe"
```

---

# Phase 5 — paths + scanner

### Task 5.1: Deny-list guard

**Files:**
- Modify: `crates/svalka-core/src/scanner.rs`

- [ ] **Step 1: Write the failing test**

`crates/svalka-core/src/scanner.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn denies_protected_locations() {
        assert!(!is_allowed(Path::new(r"C:\Windows\System32\drivers")));
        assert!(!is_allowed(Path::new(r"C:\Windows\WinSxS\x")));
        assert!(!is_allowed(Path::new(
            r"C:\Users\me\AppData\Local\Google\Chrome\User Data\Default\Cookies"
        )));
        assert!(!is_allowed(Path::new(r"C:\hiberfil.sys")));
    }

    #[test]
    fn allows_known_junk() {
        assert!(is_allowed(Path::new(r"C:\Users\me\AppData\Local\Temp\x")));
        assert!(is_allowed(Path::new(
            r"C:\Users\me\AppData\Local\Google\Chrome\User Data\Default\Cache"
        )));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core scanner::`
Expected: FAIL — `is_allowed` not found.

- [ ] **Step 3: Implement the guard**

Prepend to `scanner.rs`:
```rust
use std::path::Path;

/// Hard deny-list. Even if a category resolver returns one of these, the
/// scanner drops it. Case-insensitive substring match.
const DENY: &[&str] = &[
    r"\windows\system32",
    r"\windows\syswow64",
    r"\windows\winsxs",
    "hiberfil.sys",
    "pagefile.sys",
    "swapfile.sys",
    r"\cookies",
    r"\login data",
    r"\web data",
    r"\history",
    r"\bookmarks",
];

pub fn is_allowed(path: &Path) -> bool {
    let p = path.to_string_lossy().to_lowercase();
    !DENY.iter().any(|d| p.contains(d))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core scanner::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/scanner.rs
git commit -m "feat(core): deny-list guard for scanner"
```

### Task 5.2: Well-known roots (env-injected)

**Files:**
- Modify: `crates/svalka-core/src/paths.rs`

- [ ] **Step 1: Write the failing test**

`crates/svalka-core/src/paths.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_temp_roots_resolve_from_env() {
        let env = |k: &str| match k {
            "TEMP" => Some(r"C:\Users\me\AppData\Local\Temp".to_string()),
            "SystemRoot" => Some(r"C:\Windows".to_string()),
            _ => None,
        };
        let roots = system_temp_roots(&env);
        assert!(roots.iter().any(|p| p.ends_with("Temp")));
        assert!(roots.iter().any(|p| p.ends_with(r"Windows\Temp")));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core paths::`
Expected: FAIL — `system_temp_roots` not found.

- [ ] **Step 3: Implement env-injected resolvers**

Prepend to `paths.rs`:
```rust
use std::path::PathBuf;

/// Resolver signature: maps an env var name to its value. Injected so logic
/// is testable without touching the real environment.
pub type EnvFn<'a> = dyn Fn(&str) -> Option<String> + 'a;

pub fn live_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

pub fn system_temp_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(t) = env("TEMP") {
        v.push(PathBuf::from(t));
    }
    if let Some(w) = env("SystemRoot") {
        v.push(PathBuf::from(w).join("Temp"));
    }
    v
}

pub fn browser_cache_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(local) = env("LOCALAPPDATA") {
        let base = PathBuf::from(&local);
        for sub in [
            r"Google\Chrome\User Data\Default\Cache",
            r"Microsoft\Edge\User Data\Default\Cache",
        ] {
            v.push(base.join(sub));
        }
    }
    v
}

pub fn dev_cache_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(up) = env("USERPROFILE") {
        let base = PathBuf::from(&up);
        v.push(base.join(r".cargo\registry\cache"));
        v.push(base.join(r".gradle\caches"));
    }
    if let Some(local) = env("LOCALAPPDATA") {
        v.push(PathBuf::from(&local).join(r"pip\Cache"));
        v.push(PathBuf::from(&local).join("go-build"));
    }
    v
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core paths::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/paths.rs
git commit -m "feat(core): env-injected well-known path resolvers"
```

### Task 5.3: scan() assembles categories

**Files:**
- Modify: `crates/svalka-core/src/scanner.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `scanner.rs`:
```rust
    use crate::model::CategoryId;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scan_reports_sizes_for_existing_roots() {
        let tmp = tempdir().unwrap();
        let temp_root = tmp.path().join("Temp");
        fs::create_dir_all(&temp_root).unwrap();
        fs::write(temp_root.join("j.bin"), vec![0u8; 500]).unwrap();

        let roots = vec![(CategoryId::SystemTemp, "Системный мусор", vec![temp_root.clone()])];
        let cats = scan_roots(roots);

        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].id, CategoryId::SystemTemp);
        assert_eq!(cats[0].total(), 500);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core scanner::`
Expected: FAIL — `scan_roots` not found.

- [ ] **Step 3: Implement scan_roots and the public scan()**

Add to `scanner.rs`:
```rust
use crate::model::{Category, CategoryId, CleanItem, ItemKind};
use crate::paths;
use crate::sizer::dir_size;
use std::path::PathBuf;

/// Build categories from explicit (id, label, roots) triples. Each existing,
/// allowed root becomes one CleanItem sized on disk.
pub fn scan_roots(groups: Vec<(CategoryId, &str, Vec<PathBuf>)>) -> Vec<Category> {
    groups
        .into_iter()
        .map(|(id, label, roots)| {
            let items = roots
                .into_iter()
                .filter(|p| p.exists() && is_allowed(p))
                .map(|p| {
                    let size = dir_size(&p);
                    CleanItem { kind: ItemKind::Dir, path: p, size }
                })
                .collect();
            Category { id, label: label.to_string(), items }
        })
        .collect()
}

/// Live scan against the real environment for the requested categories.
pub fn scan(requested: &[CategoryId]) -> Vec<Category> {
    let env = paths::live_env;
    let mut groups: Vec<(CategoryId, &str, Vec<PathBuf>)> = Vec::new();
    for id in requested {
        match id {
            CategoryId::SystemTemp => {
                groups.push((*id, "Временные файлы Windows", paths::system_temp_roots(&env)))
            }
            CategoryId::BrowserCache => {
                groups.push((*id, "Кэш браузеров", paths::browser_cache_roots(&env)))
            }
            CategoryId::DevCache => {
                groups.push((*id, "Кэши разработчика", paths::dev_cache_roots(&env)))
            }
            _ => {}
        }
    }
    scan_roots(groups)
}
```

> Additional categories (RecycleBin, ThumbnailCache, CrashDumps, Wer, WindowsUpdate) attach in Task 5.4 by adding their resolvers in `paths.rs` and arms here. Keeping `scan_roots` separate is what makes this testable without env.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core scanner::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/scanner.rs
git commit -m "feat(core): scan_roots + live scan for temp/browser/dev"
```

### Task 5.4: Remaining system categories

**Files:**
- Modify: `crates/svalka-core/src/paths.rs`
- Modify: `crates/svalka-core/src/scanner.rs`

- [ ] **Step 1: Add resolvers with a test**

Add to the `tests` module in `paths.rs`:
```rust
    #[test]
    fn crash_dump_roots_resolve() {
        let env = |k: &str| match k {
            "LOCALAPPDATA" => Some(r"C:\Users\me\AppData\Local".to_string()),
            "SystemRoot" => Some(r"C:\Windows".to_string()),
            _ => None,
        };
        let roots = crash_dump_roots(&env);
        assert!(roots.iter().any(|p| p.ends_with("CrashDumps")));
        assert!(roots.iter().any(|p| p.ends_with("Minidump")));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p svalka-core paths::crash`
Expected: FAIL — `crash_dump_roots` not found.

- [ ] **Step 3: Implement the resolvers**

Add to `paths.rs`:
```rust
pub fn thumbnail_cache_roots(env: &EnvFn) -> Vec<PathBuf> {
    env("LOCALAPPDATA")
        .map(|l| vec![PathBuf::from(l).join(r"Microsoft\Windows\Explorer")])
        .unwrap_or_default()
}

pub fn crash_dump_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(l) = env("LOCALAPPDATA") {
        v.push(PathBuf::from(l).join("CrashDumps"));
    }
    if let Some(w) = env("SystemRoot") {
        v.push(PathBuf::from(w).join("Minidump"));
    }
    v
}

pub fn wer_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = env("ProgramData") {
        v.push(PathBuf::from(p).join(r"Microsoft\Windows\WER"));
    }
    if let Some(l) = env("LOCALAPPDATA") {
        v.push(PathBuf::from(l).join(r"Microsoft\Windows\WER"));
    }
    v
}

pub fn windows_update_roots(env: &EnvFn) -> Vec<PathBuf> {
    env("SystemRoot")
        .map(|w| vec![PathBuf::from(w).join(r"SoftwareDistribution\Download")])
        .unwrap_or_default()
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p svalka-core paths::`
Expected: PASS.

- [ ] **Step 5: Wire the new arms into scan()**

In `scanner.rs`, extend the `match id` in `scan()` with:
```rust
            CategoryId::ThumbnailCache => {
                groups.push((*id, "Кэш миниатюр", paths::thumbnail_cache_roots(&env)))
            }
            CategoryId::CrashDumps => {
                groups.push((*id, "Дампы падений", paths::crash_dump_roots(&env)))
            }
            CategoryId::Wer => {
                groups.push((*id, "Отчёты об ошибках", paths::wer_roots(&env)))
            }
            CategoryId::WindowsUpdate => {
                groups.push((*id, "Остатки Windows Update", paths::windows_update_roots(&env)))
            }
```

Run: `cargo build -p svalka-core`
Expected: compiles. (RecycleBin still unhandled — handled via Shell API in M2 Task 8.5; the `_ => {}` arm covers it for now.)

- [ ] **Step 6: Commit**

```bash
git add crates/svalka-core/src/paths.rs crates/svalka-core/src/scanner.rs
git commit -m "feat(core): system junk categories (thumbs, dumps, WER, update)"
```

---

# Phase 6 — programs

### Task 6.1: Parse installed programs from a registry root

**Files:**
- Modify: `crates/svalka-core/src/programs.rs`

- [ ] **Step 1: Write the failing test (uses a scratch HKCU key)**

`crates/svalka-core/src/programs.rs`:
```rust
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use serial_test::serial;
    use winreg::enums::*;
    use winreg::RegKey;

    #[test]
    #[serial]
    fn reads_program_fields_from_registry() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let base = r"Software\SvalkaTest\Uninstall";
        let (app, _) = hkcu.create_subkey(format!(r"{base}\MyApp")).unwrap();
        app.set_value("DisplayName", &"My App").unwrap();
        app.set_value("Publisher", &"Acme").unwrap();
        app.set_value("UninstallString", &r"C:\MyApp\uninstall.exe").unwrap();

        let root = hkcu.open_subkey(base).unwrap();
        let progs = read_programs_from(&root);

        hkcu.delete_subkey_all(r"Software\SvalkaTest").unwrap();

        let p = progs.iter().find(|p| p.name == "My App").expect("app present");
        assert_eq!(p.publisher.as_deref(), Some("Acme"));
        assert_eq!(p.uninstall.as_deref(), Some(r"C:\MyApp\uninstall.exe"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core programs:: -- --nocapture`
Expected: FAIL — `read_programs_from` / `Program` not found.

- [ ] **Step 3: Implement Program + reader**

Prepend to `programs.rs`:
```rust
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub enum ProgramSource {
    Registry,
    Uwp,
}

#[derive(Debug, Clone, Serialize)]
pub struct Program {
    pub name: String,
    pub publisher: Option<String>,
    pub install_location: Option<PathBuf>,
    pub uninstall: Option<String>,
    pub quiet_uninstall: Option<String>,
    pub real_size: u64,
    pub source: ProgramSource,
}

#[cfg(windows)]
pub fn read_programs_from(root: &winreg::RegKey) -> Vec<Program> {
    let mut out = Vec::new();
    for name in root.enum_keys().filter_map(Result::ok) {
        let Ok(k) = root.open_subkey(&name) else { continue };
        let display: Option<String> = k.get_value("DisplayName").ok();
        let system: u32 = k.get_value("SystemComponent").unwrap_or(0);
        let Some(display) = display else { continue };
        if system == 1 {
            continue;
        }
        let install_location: Option<String> = k.get_value("InstallLocation").ok();
        out.push(Program {
            name: display,
            publisher: k.get_value("Publisher").ok(),
            install_location: install_location
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
            uninstall: k.get_value("UninstallString").ok(),
            quiet_uninstall: k.get_value("QuietUninstallString").ok(),
            real_size: 0,
            source: ProgramSource::Registry,
        });
    }
    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core programs:: -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/programs.rs
git commit -m "feat(core): parse installed programs from a registry key"
```

### Task 6.2: list_programs() across all uninstall hives + real sizes

**Files:**
- Modify: `crates/svalka-core/src/programs.rs`

- [ ] **Step 1: Implement (integration-level; no unit test — reads the live machine)**

Add to `programs.rs`:
```rust
#[cfg(windows)]
pub fn list_programs() -> Vec<Program> {
    use winreg::enums::*;
    use winreg::RegKey;

    let sources = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];

    let mut all: Vec<Program> = Vec::new();
    for (hive, path) in sources {
        if let Ok(root) = RegKey::predef(hive).open_subkey(path) {
            all.extend(read_programs_from(&root));
        }
    }

    // dedupe by (name, publisher)
    all.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    all.dedup_by(|a, b| a.name == b.name && a.publisher == b.publisher);

    // real size from InstallLocation (the whole point — not EstimatedSize)
    for p in &mut all {
        if let Some(loc) = &p.install_location {
            p.real_size = crate::sizer::dir_size(loc);
        }
    }
    all
}

#[cfg(not(windows))]
pub fn list_programs() -> Vec<Program> {
    Vec::new()
}
```

- [ ] **Step 2: Manual smoke check**

Add a temporary example `crates/svalka-core/examples/list.rs`:
```rust
fn main() {
    for p in svalka_core::programs::list_programs() {
        println!("{:>12}  {}", p.real_size, p.name);
    }
}
```
Run: `cargo run -p svalka-core --example list`
Expected: prints your installed programs with real byte sizes. Delete the example afterward (or keep as a dev tool).

- [ ] **Step 3: Commit**

```bash
git add crates/svalka-core/src/programs.rs
git commit -m "feat(core): list_programs across hives with real folder sizes"
```

---

# Phase 7 — uninstaller + leftovers

### Task 7.1: Parse an uninstall command string

**Files:**
- Modify: `crates/svalka-core/src/uninstaller.rs`

- [ ] **Step 1: Write the failing test**

`crates/svalka-core/src/uninstaller.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_program_with_args() {
        let (prog, args) = parse_uninstall_cmd(r#""C:\App\unins000.exe" /SILENT"#);
        assert_eq!(prog, r"C:\App\unins000.exe");
        assert_eq!(args, vec!["/SILENT"]);
    }

    #[test]
    fn parses_bare_program() {
        let (prog, args) = parse_uninstall_cmd(r"C:\App\setup.exe");
        assert_eq!(prog, r"C:\App\setup.exe");
        assert!(args.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core uninstaller::`
Expected: FAIL — `parse_uninstall_cmd` not found.

- [ ] **Step 3: Implement the parser**

Prepend to `uninstaller.rs`:
```rust
use serde::Serialize;

/// Split an uninstall string into (program, args). Handles a leading quoted
/// path; otherwise takes the whole string up to the first space as the program.
pub fn parse_uninstall_cmd(cmd: &str) -> (String, Vec<String>) {
    let cmd = cmd.trim();
    if let Some(rest) = cmd.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            let prog = rest[..end].to_string();
            let args = rest[end + 1..]
                .split_whitespace()
                .map(str::to_string)
                .collect();
            return (prog, args);
        }
    }
    let mut parts = cmd.split_whitespace();
    let prog = parts.next().unwrap_or("").to_string();
    (prog, parts.map(str::to_string).collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct UninstallResult {
    pub program: String,
    pub ok: bool,
    pub message: String,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core uninstaller::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/uninstaller.rs
git commit -m "feat(core): parse uninstall command strings"
```

### Task 7.2: Run a single uninstall + a queue

**Files:**
- Modify: `crates/svalka-core/src/uninstaller.rs`

- [ ] **Step 1: Implement (spawns real uninstallers — covered by parse tests above, no unit test)**

Add to `uninstaller.rs`:
```rust
use crate::programs::Program;
use std::process::Command;

/// Run one program's uninstaller (prefers the quiet string). Blocks until it
/// exits and reports the outcome — never panics, never silently swallows.
pub fn uninstall_one(p: &Program) -> UninstallResult {
    let cmd = p.quiet_uninstall.as_ref().or(p.uninstall.as_ref());
    let Some(cmd) = cmd else {
        return UninstallResult {
            program: p.name.clone(),
            ok: false,
            message: "нет строки удаления".into(),
        };
    };
    let (prog, args) = parse_uninstall_cmd(cmd);
    match Command::new(&prog).args(&args).status() {
        Ok(s) if s.success() => UninstallResult {
            program: p.name.clone(),
            ok: true,
            message: "удалено".into(),
        },
        Ok(s) => UninstallResult {
            program: p.name.clone(),
            ok: false,
            message: format!("деинсталлятор завершился с кодом {:?}", s.code()),
        },
        Err(e) => UninstallResult {
            program: p.name.clone(),
            ok: false,
            message: format!("не удалось запустить: {e}"),
        },
    }
}

/// Run a batch sequentially, reporting each result via `on` as it completes.
pub fn uninstall_queue(progs: &[Program], on: impl Fn(&UninstallResult)) -> Vec<UninstallResult> {
    progs
        .iter()
        .map(|p| {
            let r = uninstall_one(p);
            on(&r);
            r
        })
        .collect()
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p svalka-core`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/svalka-core/src/uninstaller.rs
git commit -m "feat(core): single + batch uninstall execution"
```

### Task 7.3: Leftover name matching

**Files:**
- Modify: `crates/svalka-core/src/leftovers.rs`

- [ ] **Step 1: Write the failing test**

`crates/svalka-core/src/leftovers.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_on_product_and_publisher_tokens() {
        assert!(folder_matches("MyApp", Some("Acme"), "MyApp"));
        assert!(folder_matches("MyApp", Some("Acme"), "Acme"));
        assert!(folder_matches("My App 2024", Some("Acme Corp"), "My App"));
    }

    #[test]
    fn ignores_unrelated_and_too_short_tokens() {
        assert!(!folder_matches("MyApp", Some("Acme"), "Notepad"));
        assert!(!folder_matches("Go", None, "Google")); // 2-char token must not match
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core leftovers::`
Expected: FAIL — `folder_matches` not found.

- [ ] **Step 3: Implement matching**

Prepend to `leftovers.rs`:
```rust
use crate::model::ItemKind;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct Leftover {
    pub kind: ItemKind,
    pub path: PathBuf,
    pub size: u64,
}

/// True if `folder_name` looks related to a program identified by `product`
/// and optional `publisher`. Conservative: tokens shorter than 3 chars never
/// match (avoids "Go" hitting "Google"). Case-insensitive.
pub fn folder_matches(product: &str, publisher: Option<&str>, folder_name: &str) -> bool {
    let hay = folder_name.to_lowercase();
    let mut needles: Vec<String> = vec![product.to_lowercase()];
    if let Some(pb) = publisher {
        needles.push(pb.to_lowercase());
    }
    needles
        .iter()
        .filter(|n| n.len() >= 3)
        .any(|n| hay.contains(n.as_str()))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core leftovers::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-core/src/leftovers.rs
git commit -m "feat(core): conservative leftover folder matching"
```

### Task 7.4: find_leftovers() scans candidate roots

**Files:**
- Modify: `crates/svalka-core/src/leftovers.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `leftovers.rs`:
```rust
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn finds_matching_orphan_folder() {
        let roots_dir = tempdir().unwrap();
        let orphan = roots_dir.path().join("MyApp");
        fs::create_dir_all(&orphan).unwrap();
        fs::write(orphan.join("leftover.dat"), vec![0u8; 42]).unwrap();
        let unrelated = roots_dir.path().join("Notepad");
        fs::create_dir_all(&unrelated).unwrap();

        let found = find_leftovers_in(&[roots_dir.path().to_path_buf()], "MyApp", Some("Acme"));

        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("MyApp"));
        assert_eq!(found[0].size, 42);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p svalka-core leftovers::`
Expected: FAIL — `find_leftovers_in` not found.

- [ ] **Step 3: Implement the scanner over injected roots**

Add to `leftovers.rs`:
```rust
use crate::scanner::is_allowed;
use crate::sizer::dir_size;

/// Scan each candidate root for immediate child folders that match the program.
/// Roots are injected so this is testable; the live wrapper passes the real
/// Program Files / AppData / ProgramData dirs.
pub fn find_leftovers_in(roots: &[PathBuf], product: &str, publisher: Option<&str>) -> Vec<Leftover> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else { continue };
        for e in entries.filter_map(Result::ok) {
            let path = e.path();
            if !path.is_dir() || !is_allowed(&path) {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if folder_matches(product, publisher, &name) {
                out.push(Leftover {
                    kind: ItemKind::Dir,
                    path: path.clone(),
                    size: dir_size(&path),
                });
            }
        }
    }
    out
}

#[cfg(windows)]
pub fn find_leftovers(p: &crate::programs::Program) -> Vec<Leftover> {
    let env = crate::paths::live_env;
    let mut roots = Vec::new();
    for key in ["ProgramFiles", "ProgramFiles(x86)", "ProgramData", "LOCALAPPDATA", "APPDATA"] {
        if let Some(v) = env(key) {
            roots.push(PathBuf::from(v));
        }
    }
    find_leftovers_in(&roots, &p.name, p.publisher.as_deref())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p svalka-core leftovers::`
Expected: PASS.

- [ ] **Step 5: Run the full core suite (M1 gate)**

Run: `cargo test -p svalka-core`
Expected: ALL pass. **Milestone M1 reached: the engine is complete and tested.**

- [ ] **Step 6: Commit**

```bash
git add crates/svalka-core/src/leftovers.rs
git commit -m "feat(core): find_leftovers over candidate roots"
```

---

# Phase 8 — Tauri app (svalka-app)

### Task 8.1: Scaffold the Tauri crate

**Files:**
- Create: `crates/svalka-app/Cargo.toml`, `build.rs`, `tauri.conf.json`, `src/main.rs`
- Create: `ui/package.json`, `ui/vite.config.ts`, `ui/index.html`, `ui/src/main.ts`

- [ ] **Step 1: Initialize the UI toolchain**

Run:
```bash
cd ui
npm create vite@latest . -- --template svelte-ts
npm install
npm install @tauri-apps/api@^2
cd ..
```
Expected: a Svelte+TS Vite project in `ui/`.

- [ ] **Step 2: Create the Tauri crate manifest**

`crates/svalka-app/Cargo.toml`:
```toml
[package]
name = "svalka-app"
edition.workspace = true
version.workspace = true

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
svalka-core = { path = "../svalka-core" }
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`crates/svalka-app/build.rs`:
```rust
fn main() {
    tauri_build::build();
}
```

- [ ] **Step 3: Tauri config with admin manifest**

`crates/svalka-app/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Svalka",
  "version": "0.1.0",
  "identifier": "dev.sonic.svalka",
  "build": {
    "frontendDist": "../../ui/dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "npm --prefix ../../ui run dev",
    "beforeBuildCommand": "npm --prefix ../../ui run build"
  },
  "app": {
    "windows": [{ "title": "Svalka", "width": 980, "height": 640 }],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "nsis",
    "windows": {
      "nsis": { "installMode": "perMachine" }
    }
  }
}
```

Create `crates/svalka-app/manifest.xml` (requests admin at launch):
```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
```

In `crates/svalka-app/Cargo.toml` add the embed-manifest build dep and a `build.rs` line:
```toml
[target.'cfg(windows)'.build-dependencies]
embed-manifest = "1"
```
Update `build.rs`:
```rust
fn main() {
    #[cfg(windows)]
    {
        use embed_manifest::{embed_manifest, new_manifest};
        if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
            embed_manifest(new_manifest("Svalka")).expect("manifest");
        }
    }
    tauri_build::build();
}
```

- [ ] **Step 4: Minimal main.rs that boots**

`crates/svalka-app/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

fn main() {
    tauri::Builder::default()
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::scan,
            commands::get_programs,
            commands::quarantine_items,
            commands::list_quarantine,
            commands::restore_entry,
            commands::uninstall_programs,
            commands::find_leftovers
        ])
        .run(tauri::generate_context!())
        .expect("error running Svalka");
}
```

(Create `commands.rs` and `state.rs` in the next tasks; until then comment out the handler list to boot.)

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-app ui
git commit -m "chore(app): scaffold Tauri 2 crate + Svelte UI with admin manifest"
```

### Task 8.2: AppState

**Files:**
- Create: `crates/svalka-app/src/state.rs`

- [ ] **Step 1: Implement shared state**

`crates/svalka-app/src/state.rs`:
```rust
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
        // auto-purge >30 days on launch
        store.purge_older_than(30, now_unix());
        Self { quarantine: Mutex::new(store) }
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p svalka-app`
Expected: compiles (with `commands` still stubbed/commented).

- [ ] **Step 3: Commit**

```bash
git add crates/svalka-app/src/state.rs
git commit -m "feat(app): AppState with quarantine store + launch purge"
```

### Task 8.3: IPC commands

**Files:**
- Create: `crates/svalka-app/src/commands.rs`

- [ ] **Step 1: Implement command wrappers**

`crates/svalka-app/src/commands.rs`:
```rust
use crate::state::{now_unix, AppState};
use serde::Serialize;
use std::path::PathBuf;
use svalka_core::leftovers::{find_leftovers, Leftover};
use svalka_core::model::{Category, CategoryId};
use svalka_core::programs::{list_programs, Program};
use svalka_core::quarantine::QuarantineEntry;
use svalka_core::scanner::scan as core_scan;
use svalka_core::uninstaller::{uninstall_queue, UninstallResult};
use tauri::State;

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
    let id = format!("q{}", now_unix());
    let mut q = state.quarantine.lock().unwrap();
    match q.quarantine_paths(&id, &source, now_unix(), &volume_root, &paths, false) {
        Ok(entry) => QuarantineOutcome { ok: true, message: "в карантине".into(), freed: entry.total() },
        Err(e) => QuarantineOutcome { ok: false, message: e.to_string(), freed: 0 },
    }
}

#[tauri::command]
pub fn list_quarantine(state: State<AppState>) -> Vec<QuarantineEntry> {
    state.quarantine.lock().unwrap().list().to_vec()
}

#[tauri::command]
pub fn restore_entry(state: State<AppState>, id: String) -> Result<(), String> {
    state.quarantine.lock().unwrap().restore(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn uninstall_programs(programs: Vec<Program>) -> Vec<UninstallResult> {
    uninstall_queue(&programs, |_| {})
}

#[tauri::command]
pub fn find_leftovers_cmd(program: Program) -> Vec<Leftover> {
    find_leftovers(&program)
}
```

> Rename the handler entry in `main.rs` from `commands::find_leftovers` to `commands::find_leftovers_cmd` to match. Uncomment the full `invoke_handler` list now.

- [ ] **Step 2: Boot the app**

Run: `cargo build -p svalka-app` then `npm --prefix ui run tauri dev` (or `cargo tauri dev` if the CLI is installed).
Expected: a UAC prompt (admin manifest), then the Tauri window opens with the default Svelte page.

- [ ] **Step 3: Commit**

```bash
git add crates/svalka-app/src/commands.rs crates/svalka-app/src/main.rs
git commit -m "feat(app): IPC commands over svalka-core"
```

---

# Phase 9 — Svelte UI

### Task 9.1: Typed IPC layer + byte formatting

**Files:**
- Create: `ui/src/ipc.ts`, `ui/src/lib/format.ts`

- [ ] **Step 1: Byte formatter with a test**

`ui/src/lib/format.ts`:
```ts
export function humanBytes(n: number): string {
  if (n < 1024) return `${Math.round(n)} Б`;
  const units = ["КБ", "МБ", "ГБ", "ТБ"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(1)} ${units[i]}`;
}
```

`ui/src/lib/format.test.ts`:
```ts
import { describe, it, expect } from "vitest";
import { humanBytes } from "./format";

describe("humanBytes", () => {
  it("formats bytes, MB and GB", () => {
    expect(humanBytes(500)).toBe("500 Б");
    expect(humanBytes(1024 * 1024)).toBe("1.0 МБ");
    expect(humanBytes(3 * 1024 ** 3)).toBe("3.0 ГБ");
  });
});
```

Run: `npm --prefix ui install -D vitest && npm --prefix ui exec vitest run`
Expected: PASS.

- [ ] **Step 2: Typed IPC wrappers**

`ui/src/ipc.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";

export type CategoryId =
  | "SystemTemp" | "RecycleBin" | "ThumbnailCache" | "CrashDumps"
  | "Wer" | "WindowsUpdate" | "BrowserCache" | "DevCache";

export interface CleanItem { kind: "File" | "Dir"; path: string; size: number; }
export interface Category { id: CategoryId; label: string; items: CleanItem[]; }
export interface Program {
  name: string; publisher: string | null; install_location: string | null;
  uninstall: string | null; quiet_uninstall: string | null;
  real_size: number; source: "Registry" | "Uwp";
}
export interface MovedFile { original: string; stored: string; size: number; }
export interface QuarantineEntry {
  id: string; created: number; source: string; files: MovedFile[]; reg: unknown[];
}

export const api = {
  scan: (categories: CategoryId[]) => invoke<Category[]>("scan", { categories }),
  getPrograms: () => invoke<Program[]>("get_programs"),
  quarantineItems: (source: string, volumeRoot: string, paths: string[]) =>
    invoke<{ ok: boolean; message: string; freed: number }>("quarantine_items", {
      source, volumeRoot, paths,
    }),
  listQuarantine: () => invoke<QuarantineEntry[]>("list_quarantine"),
  restoreEntry: (id: string) => invoke<void>("restore_entry", { id }),
  uninstallPrograms: (programs: Program[]) =>
    invoke<{ program: string; ok: boolean; message: string }[]>("uninstall_programs", { programs }),
};
```

- [ ] **Step 3: Commit**

```bash
git add ui/src/ipc.ts ui/src/lib/format.ts ui/src/lib/format.test.ts ui/package.json
git commit -m "feat(ui): typed IPC layer + tested byte formatter"
```

### Task 9.2: App shell with sidebar nav

**Files:**
- Modify: `ui/src/App.svelte`

- [ ] **Step 1: Implement the sidebar shell (matches approved layout A)**

`ui/src/App.svelte`:
```svelte
<script lang="ts">
  import Clean from "./routes/Clean.svelte";
  import Programs from "./routes/Programs.svelte";
  import Quarantine from "./routes/Quarantine.svelte";
  import Settings from "./routes/Settings.svelte";

  type Tab = "clean" | "programs" | "quarantine" | "settings";
  let tab: Tab = "clean";
  const nav: { id: Tab; label: string }[] = [
    { id: "clean", label: "Очистка" },
    { id: "programs", label: "Программы" },
    { id: "quarantine", label: "Карантин" },
    { id: "settings", label: "Настройки" },
  ];
</script>

<div class="shell">
  <aside>
    <div class="brand">Svalka</div>
    {#each nav as n}
      <button class:active={tab === n.id} on:click={() => (tab = n.id)}>{n.label}</button>
    {/each}
  </aside>
  <main>
    {#if tab === "clean"}<Clean />
    {:else if tab === "programs"}<Programs />
    {:else if tab === "quarantine"}<Quarantine />
    {:else}<Settings />{/if}
  </main>
</div>

<style>
  .shell { display: flex; height: 100vh; font-family: system-ui, sans-serif; }
  aside { width: 180px; border-right: 1px solid #2222; padding: 12px; display: flex; flex-direction: column; gap: 4px; }
  .brand { font-weight: 600; padding: 8px 10px; margin-bottom: 8px; }
  aside button { text-align: left; padding: 8px 10px; border: 0; background: transparent; border-radius: 8px; cursor: pointer; }
  aside button.active { background: #3b82f6; color: #fff; }
  main { flex: 1; padding: 20px; overflow: auto; }
</style>
```

- [ ] **Step 2: Create placeholder route files so it compiles**

Create `ui/src/routes/Clean.svelte`, `Programs.svelte`, `Quarantine.svelte`, `Settings.svelte`, each:
```svelte
<h2>скоро</h2>
```

Run: `npm --prefix ui run dev` (or full `tauri dev`)
Expected: sidebar with four working tabs.

- [ ] **Step 3: Commit**

```bash
git add ui/src/App.svelte ui/src/routes
git commit -m "feat(ui): sidebar nav shell (layout A)"
```

### Task 9.3: Очистка screen

**Files:**
- Modify: `ui/src/routes/Clean.svelte`

- [ ] **Step 1: Implement scan + select + quarantine flow**

`ui/src/routes/Clean.svelte`:
```svelte
<script lang="ts">
  import { api, type Category, type CategoryId } from "../ipc";
  import { humanBytes } from "../lib/format";

  const DEFAULT: CategoryId[] = ["SystemTemp", "BrowserCache", "DevCache"];
  let cats: Category[] = [];
  let selected = new Set<string>();
  let scanning = false;
  let status = "";

  async function scan() {
    scanning = true;
    status = "Сканирую...";
    cats = await api.scan(DEFAULT);
    selected = new Set(cats.flatMap((c) => c.items.map((i) => i.path)));
    scanning = false;
    status = "";
  }

  function toggle(path: string) {
    selected.has(path) ? selected.delete(path) : selected.add(path);
    selected = selected;
  }

  $: total = cats
    .flatMap((c) => c.items)
    .filter((i) => selected.has(i.path))
    .reduce((s, i) => s + i.size, 0);

  function volumeRoot(p: string): string {
    return p.slice(0, 2) + "\\";
  }

  async function clean() {
    const paths = [...selected];
    if (!paths.length) return;
    status = "Отправляю в карантин...";
    const byVol = new Map<string, string[]>();
    for (const p of paths) {
      const v = volumeRoot(p);
      byVol.set(v, [...(byVol.get(v) ?? []), p]);
    }
    let freed = 0;
    for (const [v, ps] of byVol) {
      const r = await api.quarantineItems("Очистка", v, ps);
      if (r.ok) freed += r.freed;
    }
    status = `Освобождено ${humanBytes(freed)} — всё в карантине, можно вернуть`;
    await scan();
  }
</script>

<header class="bar">
  <strong>Очистка</strong>
  <span class="muted">{cats.length ? `Выбрано ${humanBytes(total)}` : "Нажмите «Сканировать»"}</span>
  <span class="grow"></span>
  <button on:click={scan} disabled={scanning}>Сканировать</button>
  <button on:click={clean} disabled={!selected.size}>Отправить в карантин</button>
</header>

{#if status}<p class="muted">{status}</p>{/if}

{#each cats as c}
  <section>
    <h3>{c.label} <span class="muted">{humanBytes(c.items.reduce((s, i) => s + i.size, 0))}</span></h3>
    {#each c.items as i}
      <label class="row">
        <input type="checkbox" checked={selected.has(i.path)} on:change={() => toggle(i.path)} />
        <span class="path">{i.path}</span>
        <span class="muted">{humanBytes(i.size)}</span>
      </label>
    {/each}
  </section>
{/each}

<style>
  .bar { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; }
  .grow { flex: 1; }
  .muted { color: #888; font-size: 13px; }
  .row { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px solid #2221; }
  .path { flex: 1; font-size: 13px; word-break: break-all; }
  h3 { font-size: 15px; margin: 16px 0 6px; }
  button { padding: 7px 14px; border-radius: 8px; border: 1px solid #3b82f6; background: #3b82f6; color: #fff; cursor: pointer; }
  button:disabled { opacity: 0.5; cursor: default; }
</style>
```

- [ ] **Step 2: Manual verification**

Run `tauri dev`, go to Очистка, click Сканировать.
Expected: categories with real sizes appear; unchecking rows lowers the selected total; "Отправить в карантин" reports freed bytes and the list re-scans smaller.

- [ ] **Step 3: Commit**

```bash
git add ui/src/routes/Clean.svelte
git commit -m "feat(ui): Очистка screen — scan, select, quarantine"
```

### Task 9.4: Программы screen

**Files:**
- Modify: `ui/src/routes/Programs.svelte`

- [ ] **Step 1: Implement list + batch uninstall**

`ui/src/routes/Programs.svelte`:
```svelte
<script lang="ts">
  import { api, type Program } from "../ipc";
  import { humanBytes } from "../lib/format";

  let progs: Program[] = [];
  let selected = new Set<string>();
  let loading = false;
  let status = "";

  async function load() {
    loading = true;
    progs = await api.getPrograms();
    progs.sort((a, b) => b.real_size - a.real_size);
    loading = false;
  }

  function toggle(name: string) {
    selected.has(name) ? selected.delete(name) : selected.add(name);
    selected = selected;
  }

  async function uninstall() {
    const chosen = progs.filter((p) => selected.has(p.name));
    if (!chosen.length) return;
    status = `Удаляю ${chosen.length}...`;
    const results = await api.uninstallPrograms(chosen);
    const failed = results.filter((r) => !r.ok);
    status = failed.length
      ? `Готово, но не удалось: ${failed.map((f) => `${f.program} (${f.message})`).join("; ")}`
      : "Все выбранные удалены";
    selected = new Set();
    await load();
  }

  load();
</script>

<header class="bar">
  <strong>Программы</strong>
  <span class="muted">{progs.length} шт.</span>
  <span class="grow"></span>
  <button on:click={load} disabled={loading}>Обновить</button>
  <button on:click={uninstall} disabled={!selected.size}>Удалить выбранные ({selected.size})</button>
</header>

{#if status}<p class="muted">{status}</p>{/if}

{#each progs as p}
  <label class="row">
    <input type="checkbox" checked={selected.has(p.name)} on:change={() => toggle(p.name)} />
    <span class="name">{p.name}<br /><span class="muted">{p.publisher ?? ""}</span></span>
    <span class="muted">{p.real_size ? humanBytes(p.real_size) : "—"}</span>
  </label>
{/each}

<style>
  .bar { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; }
  .grow { flex: 1; }
  .muted { color: #888; font-size: 13px; }
  .row { display: flex; align-items: center; gap: 12px; padding: 8px 0; border-bottom: 1px solid #2221; }
  .name { flex: 1; font-size: 14px; }
  button { padding: 7px 14px; border-radius: 8px; border: 1px solid #3b82f6; background: #3b82f6; color: #fff; cursor: pointer; }
  button:disabled { opacity: 0.5; cursor: default; }
</style>
```

- [ ] **Step 2: Manual verification**

Run `tauri dev`, open Программы.
Expected: programs sorted by real size (largest first); selecting a couple + "Удалить выбранные" runs their uninstallers in sequence; failures are reported, not swallowed.

- [ ] **Step 3: Commit**

```bash
git add ui/src/routes/Programs.svelte
git commit -m "feat(ui): Программы screen — real sizes + batch uninstall"
```

### Task 9.5: Карантин screen

**Files:**
- Modify: `ui/src/routes/Quarantine.svelte`

- [ ] **Step 1: Implement list + restore**

`ui/src/routes/Quarantine.svelte`:
```svelte
<script lang="ts">
  import { api, type QuarantineEntry } from "../ipc";
  import { humanBytes } from "../lib/format";

  let entries: QuarantineEntry[] = [];
  let status = "";

  async function load() {
    entries = await api.listQuarantine();
  }

  function total(e: QuarantineEntry): number {
    return e.files.reduce((s, f) => s + f.size, 0);
  }

  async function restore(id: string) {
    status = "Восстанавливаю...";
    try {
      await api.restoreEntry(id);
      status = "Восстановлено на место";
    } catch (e) {
      status = `Ошибка: ${e}`;
    }
    await load();
  }

  load();
</script>

<header class="bar">
  <strong>Карантин</strong>
  <span class="muted">{entries.length} записей · авто-очистка через 30 дней</span>
  <span class="grow"></span>
  <button on:click={load}>Обновить</button>
</header>

{#if status}<p class="muted">{status}</p>{/if}

{#if !entries.length}
  <p class="muted">Пусто. Всё удалённое появляется здесь и может быть возвращено.</p>
{/if}

{#each entries as e}
  <div class="row">
    <span class="meta">{e.source} · {e.files.length} объектов<br /><span class="muted">{humanBytes(total(e))}</span></span>
    <span class="grow"></span>
    <button on:click={() => restore(e.id)}>Восстановить</button>
  </div>
{/each}

<style>
  .bar { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; }
  .grow { flex: 1; }
  .muted { color: #888; font-size: 13px; }
  .row { display: flex; align-items: center; gap: 12px; padding: 10px 0; border-bottom: 1px solid #2221; }
  .meta { font-size: 14px; }
  button { padding: 7px 14px; border-radius: 8px; border: 1px solid #3b82f6; background: transparent; color: inherit; cursor: pointer; }
</style>
```

- [ ] **Step 2: Manual verification (the full safety loop)**

Run `tauri dev`. Clean something in Очистка → open Карантин → click Восстановить → confirm the files are back in their original location (check on disk).
Expected: restore returns the files; the entry disappears.

- [ ] **Step 3: Commit**

```bash
git add ui/src/routes/Quarantine.svelte
git commit -m "feat(ui): Карантин screen — list + restore"
```

### Task 9.6: Настройки screen

**Files:**
- Modify: `ui/src/routes/Settings.svelte`

- [ ] **Step 1: Implement a minimal settings view**

`ui/src/routes/Settings.svelte`:
```svelte
<script lang="ts">
  let purgeDays = 30;
</script>

<h2>Настройки</h2>
<label class="row">
  Авто-очистка карантина через (дней)
  <input type="number" min="1" max="365" bind:value={purgeDays} />
</label>
<p class="muted">Версия 0.1.0 · Svalka работает с правами администратора, чтобы чистить системные папки.</p>

<style>
  .row { display: flex; align-items: center; gap: 12px; margin: 16px 0; }
  .muted { color: #888; font-size: 13px; }
  input { width: 80px; padding: 6px; }
</style>
```

> Persisting `purgeDays` to disk and feeding it into the launch purge is a small follow-up: add a `get_settings`/`set_settings` command pair backed by a `settings.json` in `meta_dir()`, and pass the stored value to `purge_older_than` in `AppState::new`. Out of scope for the v1 walking skeleton.

- [ ] **Step 2: Commit**

```bash
git add ui/src/routes/Settings.svelte
git commit -m "feat(ui): minimal Настройки screen"
```

---

# Phase 10 — Integration polish

### Task 10.1: Scan progress events

**Files:**
- Modify: `crates/svalka-app/src/commands.rs`
- Modify: `ui/src/routes/Clean.svelte`

- [ ] **Step 1: Emit per-category progress**

Change `scan` in `commands.rs` to take the window and emit events:
```rust
#[tauri::command]
pub fn scan(window: tauri::Window, categories: Vec<CategoryId>) -> Vec<Category> {
    let mut out = Vec::new();
    for id in &categories {
        let cats = core_scan(std::slice::from_ref(id));
        for c in &cats {
            let _ = window.emit("scan://progress", &c.label);
        }
        out.extend(cats);
    }
    out
}
```
Add `use tauri::Emitter;` at the top of `commands.rs`.

- [ ] **Step 2: Listen in Clean.svelte**

In `Clean.svelte` `<script>`, add:
```ts
import { listen } from "@tauri-apps/api/event";
listen<string>("scan://progress", (e) => { status = `Сканирую: ${e.payload}`; });
```

- [ ] **Step 3: Manual verification**

Run `tauri dev`, click Сканировать.
Expected: status text updates per category as the scan proceeds.

- [ ] **Step 4: Commit**

```bash
git add crates/svalka-app/src/commands.rs ui/src/routes/Clean.svelte
git commit -m "feat: live scan progress events"
```

### Task 10.2: Release build smoke test

- [ ] **Step 1: Build the bundle**

Run: `npm --prefix ui run build` then `cargo tauri build` (from repo root, or `npx @tauri-apps/cli build`).
Expected: an NSIS installer + `svalka.exe` under `target/release/`. Launching prompts UAC, opens the window, all four tabs function.

- [ ] **Step 2: Final full-suite gate**

Run: `cargo test` and `npm --prefix ui exec vitest run`.
Expected: all green.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "chore: v1 walking skeleton complete"
```

---

## Self-Review

**1. Spec coverage:**
- Safe-but-transparent cleaning → Phases 5 (scan + deny-list), 4 (quarantine), Task 9.3 (per-path checkboxes). ✓
- Three categories (system/browser/dev) → Tasks 5.2–5.4. ✓
- Real program sizes → Task 6.2 (`real_size` from `dir_size`, not `EstimatedSize`). ✓
- Batch uninstall → Tasks 7.2, 9.4. ✓
- Leftover hunting (transparent) → Tasks 7.3–7.4 (returns candidates; nothing auto-deleted). ✓
- Quarantine with restore + 30-day purge → Tasks 4.2, 4.3, 8.2. ✓
- Registry export/import → Task 4.4. ✓
- No silent failures → `UninstallResult.message`, `QuarantineOutcome.message`, failures surfaced in 9.4. ✓
- Sidebar layout A → Task 9.2. ✓
- Admin at launch → Task 8.1 manifest. ✓
- **Gap found & fixed:** leftover *registry* deletion/quarantine of reg keys is modeled (`RegBackup`, `import_reg` in restore) but no task quarantines a reg key end-to-end. This is acceptable for the v1 skeleton — file leftovers are covered; reg-key quarantine reuses `export_reg` + the same entry shape and is a natural follow-up. Noted here rather than left silent.

**2. Placeholder scan:** No "TBD/TODO/implement later" in code steps. The two `> Note` blocks (Settings persistence, reg-key quarantine) are explicit scope calls with the concrete approach stated, not vague placeholders.

**3. Type consistency:** `CategoryId` enum identical across `model.rs` and `ipc.ts`. `QuarantineEntry`/`MovedFile` fields match between `quarantine.rs` and `ipc.ts`. `Program` fields match `programs.rs` ↔ `ipc.ts`. `quarantine_paths(id, source, now, volume_root, paths, dry_run)` signature is consistent between the round-trip test (4.2), purge test setup (4.3), and the command wrapper (8.3). Command name `find_leftovers_cmd` reconciled between `main.rs` and `commands.rs` in Task 8.3.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-16-svalka-cleaner.md`. Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session with checkpoints for review.

Note: the toolchain (Rust + cargo, Node + npm, Tauri CLI) must be installed on this machine before execution starts.
