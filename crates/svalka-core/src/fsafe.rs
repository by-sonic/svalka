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
        return Err(FsError::CrossVolume(
            src.to_path_buf(),
            dst_dir.to_path_buf(),
        ));
    }
    match std::fs::rename(src, &dst) {
        Ok(()) => Ok(dst),
        // 32 = ERROR_SHARING_VIOLATION, 33 = ERROR_LOCK_VIOLATION (file in use)
        Err(e) if matches!(e.raw_os_error(), Some(32) | Some(33)) => {
            Err(FsError::InUse(src.to_path_buf()))
        }
        Err(e) => Err(FsError::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn same_volume_true_for_same_drive() {
        assert!(same_volume(Path::new(r"C:\a\b"), Path::new(r"C:\c\d")));
    }

    #[test]
    fn same_volume_false_for_different_drives() {
        assert!(!same_volume(Path::new(r"C:\a"), Path::new(r"D:\b")));
    }

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
}
