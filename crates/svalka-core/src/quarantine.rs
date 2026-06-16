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
    pub files: Vec<MovedFile>,
    pub reg: Vec<RegBackup>,
}

impl QuarantineEntry {
    pub fn total(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

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
            files: vec![
                MovedFile { original: "a".into(), stored: "q/a".into(), size: 10 },
                MovedFile { original: "b".into(), stored: "q/b".into(), size: 20 },
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
}
