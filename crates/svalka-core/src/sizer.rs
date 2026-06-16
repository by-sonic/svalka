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
