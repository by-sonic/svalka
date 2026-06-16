use crate::scanner::is_allowed;
use rayon::prelude::*;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};

/// Bounds on the serialized result so a whole-drive scan stays a few hundred
/// nodes instead of millions. Sizes are always exact; only the *retained* tree
/// is bounded — deeper folders still contribute their size.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Levels of children retained below the root (sizes aggregate past this).
    pub depth: u8,
    /// Max children kept per node; the rest fold into one «остальное» node.
    pub top_children: usize,
    /// How many largest individual files to return.
    pub top_files: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { depth: 3, top_children: 16, top_files: 100 }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageNode {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
    /// `true` when this path must never be quarantined (deny-list) or is the
    /// synthetic «остальное» aggregate. The UI disables selection for these.
    pub protected: bool,
    pub children: Vec<UsageNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LargeFile {
    pub path: PathBuf,
    pub size: u64,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageReport {
    pub root: PathBuf,
    pub total: u64,
    pub free: u64,
    pub volume_total: u64,
    pub tree: UsageNode,
    pub largest: Vec<LargeFile>,
}

/// Bounded min-heap of the largest (size, path) pairs seen so far.
struct TopN {
    cap: usize,
    heap: BinaryHeap<Reverse<(u64, PathBuf)>>,
}

impl TopN {
    fn new(cap: usize) -> Self {
        Self { cap, heap: BinaryHeap::new() }
    }

    fn offer(&mut self, size: u64, path: PathBuf) {
        if self.cap == 0 {
            return;
        }
        if self.heap.len() < self.cap {
            self.heap.push(Reverse((size, path)));
        } else if let Some(Reverse((min, _))) = self.heap.peek() {
            if size > *min {
                self.heap.pop();
                self.heap.push(Reverse((size, path)));
            }
        }
    }

    fn into_pairs(self) -> Vec<(u64, PathBuf)> {
        self.heap.into_iter().map(|Reverse(x)| x).collect()
    }
}

fn basename(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// Recursively size `path`, retaining child nodes only while `depth <
/// limits.depth`. Reparse points (junction/symlink) are treated as leaves so we
/// never loop or double-count. Unreadable dirs contribute 0 and never panic.
fn scan_node(path: &Path, depth: u8, limits: Limits, top: &mut TopN) -> UsageNode {
    let sym = path.symlink_metadata();
    let is_symlink = sym
        .as_ref()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    if !is_symlink && path.is_dir() {
        let mut total: u64 = 0;
        let mut kids: Vec<UsageNode> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(path) {
            for entry in rd.flatten() {
                let child = scan_node(&entry.path(), depth + 1, limits, top);
                total += child.size;
                if depth < limits.depth {
                    kids.push(child);
                }
            }
        }
        if depth < limits.depth {
            fold_children(&mut kids, limits.top_children);
        }
        UsageNode {
            name: basename(path),
            path: path.to_path_buf(),
            size: total,
            is_dir: true,
            protected: false,
            children: kids,
        }
    } else {
        let size = sym.map(|m| m.len()).unwrap_or(0);
        top.offer(size, path.to_path_buf());
        UsageNode {
            name: basename(path),
            path: path.to_path_buf(),
            size,
            is_dir: false,
            protected: false,
            children: Vec::new(),
        }
    }
}

/// Sort children largest-first; fold everything past `top_children` into one
/// synthetic, non-actionable «остальное (N)» node.
fn fold_children(kids: &mut Vec<UsageNode>, top_children: usize) {
    kids.sort_by_key(|n| Reverse(n.size));
    if kids.len() > top_children {
        let rest = kids.split_off(top_children);
        let rest_size: u64 = rest.iter().map(|n| n.size).sum();
        let rest_count = rest.len();
        kids.push(UsageNode {
            name: format!("остальное ({rest_count})"),
            path: PathBuf::new(),
            size: rest_size,
            is_dir: false,
            protected: true,
            children: Vec::new(),
        });
    }
}

/// Set `protected` on the (bounded) retained tree. Done as a post-pass so
/// `is_allowed` (which canonicalizes — a syscall) runs a few hundred times, not
/// once per file. Skips synthetic nodes (empty path), which stay protected.
fn mark_protected(node: &mut UsageNode) {
    if !node.path.as_os_str().is_empty() {
        node.protected = !is_allowed(&node.path);
    }
    for c in &mut node.children {
        mark_protected(c);
    }
}

#[cfg(windows)]
fn disk_space(root: &Path) -> (u64, u64) {
    use std::os::windows::ffi::OsStrExt;
    // kernel32 is auto-linked by std on Windows. (Edition 2024: prefix with
    // `unsafe extern "system"` if the compiler requires it.)
    extern "system" {
        fn GetDiskFreeSpaceExW(
            lp_directory_name: *const u16,
            lp_free_bytes_available_to_caller: *mut u64,
            lp_total_number_of_bytes: *mut u64,
            lp_total_number_of_free_bytes: *mut u64,
        ) -> i32;
    }
    let wide: Vec<u16> = root.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut free_to_caller: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;
    let ok = unsafe {
        GetDiskFreeSpaceExW(wide.as_ptr(), &mut free_to_caller, &mut total, &mut total_free)
    };
    if ok != 0 {
        (total_free, total)
    } else {
        (0, 0)
    }
}

#[cfg(not(windows))]
fn disk_space(_root: &Path) -> (u64, u64) {
    (0, 0)
}

/// Analyze disk usage under `root`. Top-level children are walked in parallel
/// (rayon); `on_child` fires once per top-level child as it completes (for
/// progress). Returns a bounded size tree + the top-N largest files.
pub fn analyze<F>(root: &Path, limits: Limits, on_child: F) -> UsageReport
where
    F: Fn(&str) + Sync,
{
    let (free, volume_total) = disk_space(root);

    let empty_tree = |children: Vec<UsageNode>, size: u64| UsageNode {
        name: basename(root),
        path: root.to_path_buf(),
        size,
        is_dir: true,
        protected: false,
        children,
    };

    if !root.exists() {
        return UsageReport {
            root: root.to_path_buf(),
            total: 0,
            free,
            volume_total,
            tree: empty_tree(Vec::new(), 0),
            largest: Vec::new(),
        };
    }

    let children: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();

    let results: Vec<(UsageNode, Vec<(u64, PathBuf)>)> = children
        .par_iter()
        .map(|c| {
            let mut top = TopN::new(limits.top_files);
            let node = scan_node(c, 1, limits, &mut top);
            on_child(&node.name);
            (node, top.into_pairs())
        })
        .collect();

    let mut kids: Vec<UsageNode> = Vec::with_capacity(results.len());
    let mut global = TopN::new(limits.top_files);
    for (node, files) in results {
        kids.push(node);
        for (size, path) in files {
            global.offer(size, path);
        }
    }

    let total: u64 = kids.iter().map(|n| n.size).sum();
    fold_children(&mut kids, limits.top_children);

    let mut tree = empty_tree(kids, total);
    mark_protected(&mut tree);

    let mut pairs = global.into_pairs();
    pairs.sort_by_key(|p| Reverse(p.0));
    let largest: Vec<LargeFile> = pairs
        .into_iter()
        .map(|(size, path)| LargeFile {
            protected: !is_allowed(&path),
            path,
            size,
        })
        .collect();

    UsageReport { root: root.to_path_buf(), total, free, volume_total, tree, largest }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn noop(_: &str) {}

    #[test]
    fn total_sums_all_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.bin"), vec![0u8; 1000]).unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("b.bin"), vec![0u8; 2000]).unwrap();

        let r = analyze(dir.path(), Limits::default(), noop);
        assert_eq!(r.total, 3000);
    }

    #[test]
    fn children_sorted_desc_by_size() {
        let dir = tempdir().unwrap();
        let small = dir.path().join("small");
        let big = dir.path().join("big");
        fs::create_dir_all(&small).unwrap();
        fs::create_dir_all(&big).unwrap();
        fs::write(small.join("s"), vec![0u8; 10]).unwrap();
        fs::write(big.join("b"), vec![0u8; 9000]).unwrap();

        let r = analyze(dir.path(), Limits::default(), noop);
        assert_eq!(r.tree.children[0].name, "big");
        assert!(r.tree.children[0].size > r.tree.children[1].size);
    }

    #[test]
    fn largest_returns_top_n_desc() {
        let dir = tempdir().unwrap();
        for (name, size) in [("a", 100usize), ("b", 300), ("c", 200)] {
            fs::write(dir.path().join(name), vec![0u8; size]).unwrap();
        }
        let limits = Limits { depth: 3, top_children: 16, top_files: 2 };
        let r = analyze(dir.path(), limits, noop);
        assert_eq!(r.largest.len(), 2);
        assert_eq!(r.largest[0].size, 300);
        assert_eq!(r.largest[1].size, 200);
    }

    #[test]
    fn depth_limit_drops_deep_children_but_keeps_size() {
        let dir = tempdir().unwrap();
        let deep = dir.path().join("l1").join("l2").join("l3");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("f"), vec![0u8; 500]).unwrap();

        let limits = Limits { depth: 2, top_children: 16, top_files: 100 };
        let r = analyze(dir.path(), limits, noop);
        let l1 = &r.tree.children[0];
        assert_eq!(l1.name, "l1");
        let l2 = &l1.children[0];
        assert_eq!(l2.name, "l2");
        assert!(l2.children.is_empty(), "depth limit reached");
        assert_eq!(l2.size, 500, "size still aggregated past the limit");
    }

    #[test]
    fn folds_tail_into_other_bucket() {
        let dir = tempdir().unwrap();
        for i in 0..5usize {
            fs::write(dir.path().join(format!("f{i}")), vec![0u8; (i + 1) * 10]).unwrap();
        }
        let limits = Limits { depth: 3, top_children: 2, top_files: 100 };
        let r = analyze(dir.path(), limits, noop);
        assert_eq!(r.tree.children.len(), 3, "top 2 + остальное");
        let other = r.tree.children.last().unwrap();
        assert!(other.name.starts_with("остальное"));
        assert!(other.path.as_os_str().is_empty(), "aggregate is non-actionable");
        assert!(other.protected);
        assert_eq!(other.size, 60, "30+20+10");
    }

    #[test]
    fn protected_flag_set_for_denylisted_path() {
        let dir = tempdir().unwrap();
        let cookies = dir.path().join("Cookies");
        fs::create_dir_all(&cookies).unwrap();
        fs::write(cookies.join("x"), vec![0u8; 100]).unwrap();

        let r = analyze(dir.path(), Limits::default(), noop);
        let node = r.tree.children.iter().find(|n| n.name == "Cookies").unwrap();
        assert!(node.protected, "deny-list path must be protected");
    }

    #[test]
    fn missing_root_yields_zero_total() {
        let r = analyze(std::path::Path::new(r"Z:\nope-svalka-xyz"), Limits::default(), noop);
        assert_eq!(r.total, 0);
        assert!(r.tree.children.is_empty());
    }
}
