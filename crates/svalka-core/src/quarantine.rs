use crate::fsafe::{move_to, same_volume, FsError};
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

const STORE_DIRNAME: &str = ".svalka-quarantine";

/// A quarantine id becomes a path component, so it must never enable traversal.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
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
    /// instant). Each file goes in its own indexed subdir so identical
    /// basenames (e.g. two browsers' "Cache" folders) never collide. The whole
    /// batch is atomic: if any move or the manifest write fails, everything
    /// already moved is rolled back. `id`/`now` are injected for determinism.
    /// On `dry_run`, computes the entry without touching the filesystem.
    pub fn quarantine_paths(
        &mut self,
        id: &str,
        source: &str,
        now: u64,
        volume_root: &Path,
        paths: &[PathBuf],
        dry_run: bool,
    ) -> Result<QuarantineEntry, FsError> {
        if !is_safe_id(id) {
            return Err(FsError::Io(std::io::Error::other(format!(
                "небезопасный id карантина: {id}"
            ))));
        }
        let store_dir = volume_root.join(STORE_DIRNAME).join(id);

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
            return Ok(QuarantineEntry {
                id: id.to_string(),
                created: now,
                source: source.to_string(),
                store_dir,
                files,
                reg: Vec::new(),
            });
        }

        // Fail fast: every path must live on the quarantine volume.
        for p in paths {
            if !same_volume(p, &store_dir) {
                return Err(FsError::CrossVolume(p.clone(), store_dir.clone()));
            }
        }

        std::fs::create_dir_all(&store_dir)?;
        let mut files: Vec<MovedFile> = Vec::new();
        for (i, p) in paths.iter().enumerate() {
            let size = crate::sizer::dir_size(p);
            let sub = store_dir.join(i.to_string());
            let moved = std::fs::create_dir_all(&sub)
                .map_err(FsError::Io)
                .and_then(|_| move_to(p, &sub, false));
            match moved {
                Ok(stored) => files.push(MovedFile {
                    original: p.clone(),
                    stored,
                    size,
                }),
                Err(e) => {
                    // Roll back everything already moved in this batch.
                    for done in &files {
                        let _ = std::fs::rename(&done.stored, &done.original);
                    }
                    let _ = std::fs::remove_dir_all(&store_dir);
                    return Err(e);
                }
            }
        }

        let entry = QuarantineEntry {
            id: id.to_string(),
            created: now,
            source: source.to_string(),
            store_dir: store_dir.clone(),
            files,
            reg: Vec::new(),
        };
        self.entries.push(entry.clone());
        if let Err(e) = self.save() {
            // Manifest write failed: undo the moves so files are never left
            // orphaned (moved off disk but unrecorded, hence unrestorable).
            self.entries.pop();
            for f in &entry.files {
                let _ = std::fs::rename(&f.stored, &f.original);
            }
            let _ = std::fs::remove_dir_all(&entry.store_dir);
            return Err(FsError::Io(e));
        }
        Ok(entry)
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

    /// Delete entries whose `created` is older than `days` before `now`
    /// (both in unix seconds). Removes stored files and returns the count.
    pub fn purge_older_than(&mut self, days: u64, now: u64) -> u64 {
        let cutoff = now.saturating_sub(days.saturating_mul(86_400));
        let mut removed = 0u64;
        self.entries.retain(|e| {
            if e.created < cutoff {
                if !e.store_dir.as_os_str().is_empty() {
                    let _ = std::fs::remove_dir_all(&e.store_dir);
                }
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
            .unwrap();

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
}
