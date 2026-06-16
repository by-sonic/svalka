use crate::model::ItemKind;
use crate::scanner::is_allowed;
use crate::sizer::dir_size;
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
/// match (avoids "Go" hitting "Google"). The product and publisher are split
/// into whitespace tokens so a folder named after a subset of the product
/// ("My App" for "My App 2024") still matches. Case-insensitive.
pub fn folder_matches(product: &str, publisher: Option<&str>, folder_name: &str) -> bool {
    let hay = folder_name.to_lowercase();
    let mut sources: Vec<String> = vec![product.to_lowercase()];
    if let Some(pb) = publisher {
        sources.push(pb.to_lowercase());
    }
    sources
        .iter()
        .flat_map(|s| s.split_whitespace())
        .filter(|t| t.len() >= 3)
        .any(|t| hay.contains(t))
}

/// Scan each candidate root for immediate child folders that match the program.
/// Roots are injected so this is testable; the live wrapper passes the real
/// Program Files / AppData / ProgramData dirs.
pub fn find_leftovers_in(
    roots: &[PathBuf],
    product: &str,
    publisher: Option<&str>,
) -> Vec<Leftover> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
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
    for key in [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "LOCALAPPDATA",
        "APPDATA",
    ] {
        if let Some(v) = env(key) {
            roots.push(PathBuf::from(v));
        }
    }
    find_leftovers_in(&roots, &p.name, p.publisher.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

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
}
