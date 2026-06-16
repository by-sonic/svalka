use crate::model::{Category, CategoryId, CleanItem, ItemKind};
use crate::paths;
use crate::sizer::dir_size;
use std::path::{Path, PathBuf};

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
    // Canonicalize first so 8.3 short names and junctions/symlinks aimed at a
    // protected location can't slip past the substring deny-list. Falls back to
    // the literal path when canonicalization fails (e.g. the path is gone).
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let p = canonical.to_string_lossy().to_lowercase();
    !DENY.iter().any(|d| p.contains(d))
}

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
                    CleanItem {
                        kind: ItemKind::Dir,
                        path: p,
                        size,
                    }
                })
                .collect();
            Category {
                id,
                label: label.to_string(),
                items,
            }
        })
        .collect()
}

/// Live scan against the real environment for the requested categories.
pub fn scan(requested: &[CategoryId]) -> Vec<Category> {
    let env = paths::live_env;
    let mut groups: Vec<(CategoryId, &str, Vec<PathBuf>)> = Vec::new();
    for id in requested {
        match id {
            CategoryId::SystemTemp => groups.push((
                *id,
                "Временные файлы Windows",
                paths::system_temp_roots(&env),
            )),
            CategoryId::BrowserCache => {
                groups.push((*id, "Кэш браузеров", paths::browser_cache_roots(&env)))
            }
            CategoryId::DevCache => {
                groups.push((*id, "Кэши разработчика", paths::dev_cache_roots(&env)))
            }
            CategoryId::ThumbnailCache => {
                groups.push((*id, "Кэш миниатюр", paths::thumbnail_cache_roots(&env)))
            }
            CategoryId::CrashDumps => {
                groups.push((*id, "Дампы падений", paths::crash_dump_roots(&env)))
            }
            CategoryId::Wer => groups.push((*id, "Отчёты об ошибках", paths::wer_roots(&env))),
            CategoryId::WindowsUpdate => groups.push((
                *id,
                "Остатки Windows Update",
                paths::windows_update_roots(&env),
            )),
            // Корзина чистится через Shell API в приложении (M2), не по путям.
            // Явная ветка вместо `_` — добавление новой категории вызовет
            // ошибку компиляции, а не молчаливый пропуск.
            CategoryId::RecycleBin => {}
        }
    }
    scan_roots(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CategoryId;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

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

    #[test]
    fn scan_reports_sizes_for_existing_roots() {
        let tmp = tempdir().unwrap();
        let temp_root = tmp.path().join("Temp");
        fs::create_dir_all(&temp_root).unwrap();
        fs::write(temp_root.join("j.bin"), vec![0u8; 500]).unwrap();

        let roots = vec![(
            CategoryId::SystemTemp,
            "Системный мусор",
            vec![temp_root.clone()],
        )];
        let cats = scan_roots(roots);

        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].id, CategoryId::SystemTemp);
        assert_eq!(cats[0].total(), 500);
    }
}
