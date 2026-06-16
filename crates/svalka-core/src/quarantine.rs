use crate::fsafe::{move_to, FsError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    /// Root dir holding this entry's moved files: `<volume>\.svalka-quarantine\<id>`.
    /// `#[serde(default)]` keeps older manifests (without this field) loadable.
    #[serde(default)]
    pub store_dir: PathBuf,
    pub files: Vec<MovedFile>,
    pub reg: Vec<RegBackup>,
}

impl QuarantineEntry {
    pub fn total(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

/// An item that could not be moved into quarantine (in use, missing,
/// permission denied, …). Reported per-item instead of failing the whole batch
/// — for a junk cleaner a locked file is the normal case, not a fatal error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedItem {
    pub path: PathBuf,
    pub reason: String,
}

/// Outcome of a quarantine batch: `entry` holds what actually moved (its
/// `files` is empty when nothing could be moved), `skipped` lists what didn't
/// and why.
#[derive(Debug, Clone)]
pub struct QuarantineReport {
    pub entry: QuarantineEntry,
    pub skipped: Vec<SkippedItem>,
}

const STORE_DIRNAME: &str = ".svalka-quarantine";

/// A quarantine id becomes a path component, so it must never enable traversal.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Move `src` (a file or a whole directory) into a fresh numbered subdir of
/// `store_dir`. Fast path: one atomic rename of the whole thing. If that fails
/// and `src` is a real directory — almost always because a file inside is
/// locked/in-use (an open browser holds its cache, a process holds a temp
/// file) — descend and move the children individually, so everything movable
/// is still reclaimed and only the genuinely locked leaves are skipped. Moved
/// units land in `files`; unmovable leaves in `skipped`.
fn quarantine_tree(
    src: &Path,
    store_dir: &Path,
    seq: &mut u64,
    files: &mut Vec<MovedFile>,
    skipped: &mut Vec<SkippedItem>,
) {
    let sub = store_dir.join(seq.to_string());
    *seq += 1;
    let moved = std::fs::create_dir_all(&sub)
        .map_err(FsError::Io)
        .and_then(|_| move_to(src, &sub, false));
    match moved {
        Ok(stored) => {
            let size = crate::sizer::dir_size(&stored);
            files.push(MovedFile {
                original: src.to_path_buf(),
                stored,
                size,
            });
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&sub);
            // Only descend into a *real* directory. A reparse point (junction/
            // symlink) is moved as a unit by the fast path; never walk into its
            // target.
            let is_real_dir = src.is_dir()
                && !src
                    .symlink_metadata()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
            if is_real_dir {
                match std::fs::read_dir(src) {
                    Ok(rd) => {
                        for child in rd.flatten() {
                            quarantine_tree(&child.path(), store_dir, seq, files, skipped);
                        }
                        // If every child moved out, drop the now-empty dir so the
                        // folder is actually cleaned (mirrors the fast path, which
                        // removes the whole dir). Still-occupied => left as-is.
                        let _ = std::fs::remove_dir(src);
                    }
                    Err(re) => skipped.push(SkippedItem {
                        path: src.to_path_buf(),
                        reason: re.to_string(),
                    }),
                }
            } else {
                skipped.push(SkippedItem {
                    path: src.to_path_buf(),
                    reason: e.to_string(),
                });
            }
        }
    }
}

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

    /// Persist the manifest. Returns the error so callers can keep disk state
    /// and manifest consistent instead of silently diverging.
    fn save(&self) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(&self.entries)
            .map_err(|e| std::io::Error::other(format!("сериализация манифеста: {e}")))?;
        std::fs::write(Self::manifest_path(&self.meta_dir), json)
    }

    pub fn list(&self) -> &[QuarantineEntry] {
        &self.entries
    }

    /// Move `paths` into `volume_root\.svalka-quarantine\<id>` (same-volume,
    /// instant). Each moved unit gets its own numbered subdir so identical
    /// basenames never collide. Per-item resilient: a locked/in-use leaf is
    /// skipped and reported, never fatal; directories that can't move as a unit
    /// are descended into so most of their contents are still reclaimed. The
    /// manifest write stays atomic — if it fails, every move is rolled back so
    /// nothing is left moved-but-unrecorded. `id`/`now` injected for
    /// determinism. On `dry_run`, computes the entry without touching disk.
    pub fn quarantine_paths(
        &mut self,
        id: &str,
        source: &str,
        now: u64,
        volume_root: &Path,
        paths: &[PathBuf],
        dry_run: bool,
    ) -> Result<QuarantineReport, FsError> {
        if !is_safe_id(id) {
            return Err(FsError::Io(std::io::Error::other(format!(
                "небезопасный id карантина: {id}"
            ))));
        }
        let store_dir = volume_root.join(STORE_DIRNAME).join(id);

        let mk_entry = |files: Vec<MovedFile>| QuarantineEntry {
            id: id.to_string(),
            created: now,
            source: source.to_string(),
            store_dir: store_dir.clone(),
            files,
            reg: Vec::new(),
        };

        if dry_run {
            let files = paths
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let sub = store_dir.join(i.to_string());
                    let stored = move_to(p, &sub, true).unwrap_or_else(|_| sub.join("item"));
                    MovedFile {
                        original: p.clone(),
                        stored,
                        size: crate::sizer::dir_size(p),
                    }
                })
                .collect();
            return Ok(QuarantineReport {
                entry: mk_entry(files),
                skipped: Vec::new(),
            });
        }

        // Creating the per-volume store is the one hard prerequisite: if it
        // fails (e.g. the process isn't elevated and the volume root is
        // read-only) nothing on this volume can be quarantined, so surface that
        // as a real error rather than a silent no-op.
        std::fs::create_dir_all(&store_dir)?;

        // Move each selected item. Directories are descended into when they
        // can't move as a unit, so a single locked file (e.g. an open browser's
        // cache) only skips that file — not the whole folder.
        let mut files: Vec<MovedFile> = Vec::new();
        let mut skipped: Vec<SkippedItem> = Vec::new();
        let mut seq: u64 = 0;
        for p in paths {
            quarantine_tree(p, &store_dir, &mut seq, &mut files, &mut skipped);
        }

        // Nothing moved: don't record an empty entry or leave an empty store
        // dir littering the volume root.
        if files.is_empty() {
            let _ = std::fs::remove_dir_all(&store_dir);
            return Ok(QuarantineReport {
                entry: mk_entry(files),
                skipped,
            });
        }

        let entry = mk_entry(files);
        self.entries.push(entry.clone());
        if let Err(e) = self.save() {
            // Manifest write failed: undo the moves so files are never left
            // orphaned (moved off disk but unrecorded, hence unrestorable).
            self.entries.pop();
            for f in &entry.files {
                if let Some(parent) = f.original.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::rename(&f.stored, &f.original);
            }
            let _ = std::fs::remove_dir_all(&entry.store_dir);
            return Err(FsError::Io(e));
        }
        Ok(QuarantineReport { entry, skipped })
    }

    /// Move every file in the entry back to its original path, re-import any
    /// registry backups, then drop the entry. Refuses to clobber: if anything
    /// already occupies an original path, the whole restore aborts untouched.
    pub fn restore(&mut self, entry_id: &str) -> Result<(), FsError> {
        let idx = self
            .entries
            .iter()
            .position(|e| e.id == entry_id)
            .ok_or_else(|| FsError::Io(std::io::Error::other("запись карантина не найдена")))?;
        let entry = self.entries[idx].clone();

        for f in &entry.files {
            if f.original.exists() {
                return Err(FsError::Io(std::io::Error::other(format!(
                    "восстановление отменено: путь уже занят: {}",
                    f.original.display()
                ))));
            }
        }
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
        if !entry.store_dir.as_os_str().is_empty() {
            let _ = std::fs::remove_dir_all(&entry.store_dir);
        }
        self.entries.remove(idx);
        self.save().map_err(FsError::Io)?;
        Ok(())
    }

    /// Erase an entry's stored files from disk. Permanent.
    fn erase_stored(entry: &QuarantineEntry) {
        if !entry.store_dir.as_os_str().is_empty() {
            let _ = std::fs::remove_dir_all(&entry.store_dir);
        }
        for f in &entry.files {
            let _ = std::fs::remove_dir_all(&f.stored);
            let _ = std::fs::remove_file(&f.stored);
        }
    }

    /// Permanently delete one quarantined entry (no restore). Irreversible.
    pub fn delete(&mut self, entry_id: &str) -> Result<(), FsError> {
        let idx = self
            .entries
            .iter()
            .position(|e| e.id == entry_id)
            .ok_or_else(|| FsError::Io(std::io::Error::other("запись карантина не найдена")))?;
        let entry = self.entries.remove(idx);
        Self::erase_stored(&entry);
        self.save().map_err(FsError::Io)?;
        Ok(())
    }

    /// Permanently empty the whole quarantine. Returns how many entries went.
    pub fn clear(&mut self) -> Result<u64, FsError> {
        let n = self.entries.len() as u64;
        for e in &self.entries {
            Self::erase_stored(e);
        }
        self.entries.clear();
        self.save().map_err(FsError::Io)?;
        Ok(n)
    }

    /// Delete entries whose `created` is older than `days` before `now`
    /// (both in unix seconds). Removes stored files and returns the count.
    pub fn purge_older_than(&mut self, days: u64, now: u64) -> u64 {
        let cutoff = now.saturating_sub(days.saturating_mul(86_400));
        let mut removed = 0u64;
        self.entries.retain(|e| {
            if e.created < cutoff {
                Self::erase_stored(e);
                removed += 1;
                false
            } else {
                true
            }
        });
        if removed > 0 {
            let _ = self.save();
        }
        removed
    }

    #[cfg(test)]
    pub fn push_for_test(&mut self, e: QuarantineEntry) {
        self.entries.push(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn entry_total_sums_moved_files() {
        let entry = QuarantineEntry {
            id: "e1".into(),
            created: 0,
            source: "Очистка".into(),
            store_dir: PathBuf::new(),
            files: vec![
                MovedFile {
                    original: "a".into(),
                    stored: "q/a".into(),
                    size: 10,
                },
                MovedFile {
                    original: "b".into(),
                    stored: "q/b".into(),
                    size: 20,
                },
            ],
            reg: vec![],
        };
        assert_eq!(entry.total(), 30);
    }

    #[test]
    fn purge_removes_entries_older_than_cutoff() {
        let meta = tempdir().unwrap();
        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        store.push_for_test(QuarantineEntry {
            id: "old".into(),
            created: 0,
            source: "x".into(),
            store_dir: PathBuf::new(),
            files: vec![],
            reg: vec![],
        });
        store.push_for_test(QuarantineEntry {
            id: "new".into(),
            created: 100_000,
            source: "x".into(),
            store_dir: PathBuf::new(),
            files: vec![],
            reg: vec![],
        });

        let day = 86_400u64;
        let removed = store.purge_older_than(1, day + 50_000);

        assert_eq!(removed, 1);
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].id, "new");
    }

    #[test]
    fn quarantine_then_restore_handles_same_basename_collision() {
        let vol = tempdir().unwrap();
        let meta = tempdir().unwrap();

        // Two different dirs whose basename is identical ("Cache").
        let a = vol.path().join("ChromeProfile").join("Cache");
        let b = vol.path().join("EdgeProfile").join("Cache");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("x.bin"), b"aaaa").unwrap();
        std::fs::write(b.join("y.bin"), b"bbbbbb").unwrap();

        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        let entry = store
            .quarantine_paths(
                "c1",
                "Очистка",
                1,
                vol.path(),
                &[a.clone(), b.clone()],
                false,
            )
            .unwrap()
            .entry;

        assert!(!a.exists() && !b.exists(), "both moved out");
        assert_eq!(entry.total(), 10, "4 + 6 bytes, no collision loss");
        assert_ne!(
            entry.files[0].stored, entry.files[1].stored,
            "stored paths distinct"
        );

        store.restore("c1").unwrap();

        assert_eq!(std::fs::read(a.join("x.bin")).unwrap(), b"aaaa");
        assert_eq!(std::fs::read(b.join("y.bin")).unwrap(), b"bbbbbb");
        assert!(store.list().is_empty());
    }

    #[test]
    fn restore_refuses_to_clobber_existing_path() {
        let vol = tempdir().unwrap();
        let meta = tempdir().unwrap();
        let junk = vol.path().join("d").join("f.bin");
        std::fs::create_dir_all(junk.parent().unwrap()).unwrap();
        std::fs::write(&junk, b"orig").unwrap();

        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        store
            .quarantine_paths(
                "r1",
                "Очистка",
                1,
                vol.path(),
                std::slice::from_ref(&junk),
                false,
            )
            .unwrap();

        // Something new now occupies the original path.
        std::fs::write(&junk, b"new-occupant").unwrap();

        assert!(store.restore("r1").is_err(), "must not overwrite occupant");
        assert_eq!(
            std::fs::read(&junk).unwrap(),
            b"new-occupant",
            "occupant intact"
        );
        assert_eq!(store.list().len(), 1, "entry kept for retry");
    }

    #[test]
    fn rejects_unsafe_id() {
        let vol = tempdir().unwrap();
        let meta = tempdir().unwrap();
        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        let r = store.quarantine_paths(r"..\escape", "x", 1, vol.path(), &[], false);
        assert!(r.is_err());
    }

    #[test]
    fn one_unmovable_item_does_not_doom_the_batch() {
        let vol = tempdir().unwrap();
        let meta = tempdir().unwrap();

        let good = vol.path().join("good").join("g.bin");
        std::fs::create_dir_all(good.parent().unwrap()).unwrap();
        std::fs::write(&good, b"keep").unwrap();
        // A path that can't be moved (it doesn't exist) stands in for the
        // real-world locked/in-use file (an open browser holding its cache).
        let bad = vol.path().join("missing.bin");

        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        let report = store
            .quarantine_paths(
                "b1",
                "Очистка",
                1,
                vol.path(),
                &[good.clone(), bad.clone()],
                false,
            )
            .expect("a single bad item must not turn the whole batch into an error");

        assert!(!good.exists(), "good item should be quarantined");
        assert_eq!(report.entry.files.len(), 1, "exactly one moved");
        assert_eq!(report.skipped.len(), 1, "exactly one skipped");
        assert_eq!(report.skipped[0].path, bad);
        assert_eq!(store.list().len(), 1, "entry persisted for the moved item");

        store.restore("b1").unwrap();
        assert!(good.exists(), "good item restored to its place");
    }

    #[test]
    fn all_items_unmovable_records_no_entry() {
        let vol = tempdir().unwrap();
        let meta = tempdir().unwrap();
        let bad = vol.path().join("missing.bin");

        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        let report = store
            .quarantine_paths("b2", "Очистка", 1, vol.path(), std::slice::from_ref(&bad), false)
            .unwrap();

        assert!(report.entry.files.is_empty(), "nothing moved");
        assert_eq!(report.skipped.len(), 1, "the one item is reported");
        assert!(store.list().is_empty(), "no empty entry recorded");
        assert!(
            !vol.path().join(STORE_DIRNAME).join("b2").exists(),
            "empty store dir cleaned up"
        );
    }

    #[test]
    fn directory_roundtrip_moves_and_restores_tree() {
        let vol = tempdir().unwrap();
        let meta = tempdir().unwrap();
        let dir = vol.path().join("Temp");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.bin"), b"aaa").unwrap();
        std::fs::write(dir.join("sub").join("b.bin"), b"bbbb").unwrap();

        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        let report = store
            .quarantine_paths("d1", "Очистка", 1, vol.path(), std::slice::from_ref(&dir), false)
            .unwrap();

        assert!(!dir.exists(), "whole dir moved out");
        assert_eq!(report.entry.total(), 7);

        store.restore("d1").unwrap();
        assert_eq!(std::fs::read(dir.join("a.bin")).unwrap(), b"aaa");
        assert_eq!(std::fs::read(dir.join("sub").join("b.bin")).unwrap(), b"bbbb");
    }

    #[test]
    fn moves_dir_contents_when_whole_dir_is_blocked() {
        let vol = tempdir().unwrap();
        let meta = tempdir().unwrap();
        let dir = vol.path().join("Temp");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.bin"), b"aaa").unwrap();
        std::fs::write(dir.join("sub").join("b.bin"), b"bbbb").unwrap();

        // Force the whole-dir fast move to fail by pre-occupying its
        // destination with a non-empty dir (rename onto it errors on every
        // platform), so the engine must descend and move children individually
        // — the real-world "a file inside is locked" path.
        let store_dir = vol.path().join(STORE_DIRNAME).join("r1");
        std::fs::create_dir_all(store_dir.join("0").join("Temp")).unwrap();
        std::fs::write(store_dir.join("0").join("Temp").join("block"), b"x").unwrap();

        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        let report = store
            .quarantine_paths("r1", "Очистка", 1, vol.path(), std::slice::from_ref(&dir), false)
            .unwrap();

        assert!(report.entry.files.len() >= 2, "descended into children");
        assert!(!dir.join("a.bin").exists(), "child moved out");
        assert_eq!(report.entry.total(), 7, "all bytes reclaimed");

        store.restore("r1").unwrap();
        assert_eq!(std::fs::read(dir.join("a.bin")).unwrap(), b"aaa");
        assert_eq!(std::fs::read(dir.join("sub").join("b.bin")).unwrap(), b"bbbb");
    }

    #[test]
    fn delete_removes_entry_and_its_files() {
        let vol = tempdir().unwrap();
        let meta = tempdir().unwrap();
        let junk = vol.path().join("j.bin");
        std::fs::write(&junk, b"bytes").unwrap();

        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        let report = store
            .quarantine_paths("x1", "Очистка", 1, vol.path(), std::slice::from_ref(&junk), false)
            .unwrap();
        let stored = report.entry.files[0].stored.clone();
        assert!(stored.exists());

        store.delete("x1").unwrap();
        assert!(store.list().is_empty(), "entry dropped");
        assert!(!stored.exists(), "stored file erased");
        assert!(!junk.exists(), "permanent delete does not restore");
    }

    #[test]
    fn clear_empties_the_whole_quarantine() {
        let vol = tempdir().unwrap();
        let meta = tempdir().unwrap();
        let a = vol.path().join("a.bin");
        let b = vol.path().join("b.bin");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();

        let mut store = QuarantineStore::new(meta.path().to_path_buf());
        store.quarantine_paths("c1", "x", 1, vol.path(), std::slice::from_ref(&a), false).unwrap();
        store.quarantine_paths("c2", "x", 1, vol.path(), std::slice::from_ref(&b), false).unwrap();
        assert_eq!(store.list().len(), 2);

        assert_eq!(store.clear().unwrap(), 2);
        assert!(store.list().is_empty());
    }
}
