# Disk Analyzer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a fifth screen «Анализ диска» — a disk-usage treemap with folder drill-down and a largest-files finder, where removal reuses the existing reversible quarantine.

**Architecture:** New headless core module `usage.rs` does one parallel walk (rayon over top-level children, recursive within) producing a depth-bounded size tree + a top-N largest-files heap, with `protected` flags from the existing `scanner::is_allowed`. A thin `analyze_disk` Tauri command runs it off the main thread (`spawn_blocking`) and emits progress; deletion reuses the existing `quarantine_items` verbatim. A new `Usage.svelte` screen renders the treemap + list and wires selection → quarantine.

**Tech Stack:** Rust (`jwalk`/`rayon` already deps, raw `GetDiskFreeSpaceExW` FFI for free space), Tauri 2, Svelte 5 + TypeScript.

**Spec:** `docs/superpowers/specs/2026-06-16-disk-analyzer-design.md`

**Convention:** every commit ends with the trailer
`Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

---

## File Structure

- **Create** `crates/svalka-core/src/usage.rs` — usage model (`UsageNode`, `LargeFile`, `UsageReport`, `Limits`), the parallel walk (`analyze`), helpers (`scan_node`, `fold_children`, `mark_protected`, `disk_space`), and unit tests. Sole responsibility: size a tree + find the largest files, safely and bounded.
- **Modify** `crates/svalka-core/src/lib.rs` — register `pub mod usage;`.
- **Modify** `crates/svalka-app/src/commands.rs` — add `analyze_disk` + `list_drives` commands.
- **Modify** `crates/svalka-app/src/main.rs` — register the two commands.
- **Modify** `ui/src/ipc.ts` — `UsageNode`/`LargeFile`/`UsageReport` types + `analyzeDisk`/`listDrives`.
- **Modify** `ui/src/lib/Icon.svelte` — add `analyze` icon.
- **Modify** `ui/src/App.svelte` — add `"usage"` tab (nav + route).
- **Create** `ui/src/routes/Usage.svelte` — the screen.

**Testing strategy (matches the codebase):** the core is fully unit-tested; the UI has no component harness (only `format.test.ts`), so UI tasks verify via typecheck/build + a manual smoke run — the project's existing pattern. TDD rigor lives in Task 1.

---

## Task 1: Core module `usage.rs`

**Files:**
- Create: `crates/svalka-core/src/usage.rs`
- Modify: `crates/svalka-core/src/lib.rs:10` (add module line)
- Test: in-file `#[cfg(test)]` module (project convention)

- [ ] **Step 1: Register the module**

Edit `crates/svalka-core/src/lib.rs` — add after `pub mod uninstaller;`:

```rust
pub mod uninstaller;
pub mod usage;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/svalka-core/src/usage.rs` with ONLY the test module first (it won't compile yet — that's the failing state):

```rust
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
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p svalka-core usage`
Expected: FAIL — compile errors (`cannot find function analyze`, `Limits`, etc.).

- [ ] **Step 4: Write the implementation**

Prepend the implementation ABOVE the test module in `crates/svalka-core/src/usage.rs`:

```rust
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
    kids.sort_by(|a, b| b.size.cmp(&a.size));
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
    pairs.sort_by(|a, b| b.0.cmp(&a.0));
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p svalka-core usage`
Expected: PASS — all 7 tests green.
(If the compiler errors that the `extern` block must be `unsafe extern`, you're on edition 2024 — add the `unsafe` keyword before `extern "system"`.)

- [ ] **Step 6: Lint**

Run: `cargo clippy -p svalka-core --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/svalka-core/src/usage.rs crates/svalka-core/src/lib.rs
git commit -m "feat(core): disk-usage analyzer (bounded size tree + top-N largest files)" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: App commands `analyze_disk` + `list_drives`

**Files:**
- Modify: `crates/svalka-app/src/commands.rs` (imports near top + new commands at end)
- Modify: `crates/svalka-app/src/main.rs:9-20` (invoke_handler list)

- [ ] **Step 1: Add the import**

In `crates/svalka-app/src/commands.rs`, add to the `svalka_core::` use group (next to the other `use svalka_core::...` lines near the top):

```rust
use svalka_core::usage::{analyze, Limits, UsageReport};
```

- [ ] **Step 2: Add the commands**

Append to the end of `crates/svalka-app/src/commands.rs`:

```rust
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
```

- [ ] **Step 3: Register the commands**

In `crates/svalka-app/src/main.rs`, extend the `generate_handler!` list — add the two entries after `commands::find_leftovers_cmd,`:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::scan,
            commands::get_programs,
            commands::quarantine_items,
            commands::list_quarantine,
            commands::restore_entry,
            commands::delete_entry,
            commands::clear_quarantine,
            commands::uninstall_programs,
            commands::find_leftovers_cmd,
            commands::analyze_disk,
            commands::list_drives,
            commands::open_url
        ])
```

- [ ] **Step 4: Build to verify it compiles**

Run: `cargo build -p svalka-app`
Expected: compiles (warnings about an unused `free_to_caller` binding are not emitted — it's written by the FFI call).

- [ ] **Step 5: Commit**

```bash
git add crates/svalka-app/src/commands.rs crates/svalka-app/src/main.rs
git commit -m "feat(app): analyze_disk + list_drives commands" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: UI IPC bindings

**Files:**
- Modify: `ui/src/ipc.ts` (add interfaces after `QuarantineOutcome`; add methods to `api`)

- [ ] **Step 1: Add the types**

In `ui/src/ipc.ts`, after the `QuarantineOutcome` interface (before `export const api`):

```ts
export interface UsageNode {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
  protected: boolean;
  children: UsageNode[];
}
export interface LargeFile {
  path: string;
  size: number;
  protected: boolean;
}
export interface UsageReport {
  root: string;
  total: number;
  free: number;
  volume_total: number;
  tree: UsageNode;
  largest: LargeFile[];
}
```

- [ ] **Step 2: Add the api methods**

In the `api` object in `ui/src/ipc.ts`, add after the `findLeftovers` entry:

```ts
  analyzeDisk: (root: string) => invoke<UsageReport>("analyze_disk", { root }),
  listDrives: () => invoke<string[]>("list_drives"),
```

- [ ] **Step 3: Typecheck**

Run: `npm --prefix ui run build`
Expected: builds (no TS errors). Usage.svelte doesn't exist yet, but nothing imports it — this just confirms the IPC changes typecheck.

- [ ] **Step 4: Commit**

```bash
git add ui/src/ipc.ts
git commit -m "feat(ui): IPC bindings for disk analyzer" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Icon + routing

**Files:**
- Modify: `ui/src/lib/Icon.svelte` (add `analyze` branch)
- Modify: `ui/src/App.svelte` (Tab union, nav array, route, import)

- [ ] **Step 1: Add the `analyze` icon**

In `ui/src/lib/Icon.svelte`, add a branch before the closing `{/if}` (after the `logo` branch):

```svelte
  {:else if name === "analyze"}
    <rect x="3" y="3" width="8" height="11" rx="1.5" />
    <rect x="13" y="3" width="8" height="6" rx="1.5" />
    <rect x="13" y="11" width="8" height="10" rx="1.5" />
    <rect x="3" y="16" width="8" height="5" rx="1.5" />
```

- [ ] **Step 2: Import the screen and extend the Tab union**

In `ui/src/App.svelte`, add the import next to the other route imports:

```svelte
  import Usage from "./routes/Usage.svelte";
```

Change the `Tab` type and add the nav entry:

```svelte
  type Tab = "clean" | "programs" | "usage" | "quarantine" | "settings";
  let tab: Tab = "clean";
  let showDonate = false;

  const nav: { id: Tab; label: string; icon: string }[] = [
    { id: "clean", label: "Очистка", icon: "clean" },
    { id: "programs", label: "Программы", icon: "programs" },
    { id: "usage", label: "Анализ диска", icon: "analyze" },
    { id: "quarantine", label: "Карантин", icon: "quarantine" },
    { id: "settings", label: "Настройки", icon: "settings" },
  ];
```

- [ ] **Step 3: Add the route branch**

In `ui/src/App.svelte`, update the `{#if}` chain inside `{#key tab}`:

```svelte
        {#if tab === "clean"}<Clean />
        {:else if tab === "programs"}<Programs />
        {:else if tab === "usage"}<Usage />
        {:else if tab === "quarantine"}<Quarantine />
        {:else}<Settings onDonate={() => (showDonate = true)} />{/if}
```

- [ ] **Step 4: Typecheck**

Run: `npm --prefix ui run build`
Expected: builds — Usage.svelte is created in Task 5, so do Steps 1–3 here but run the build only AFTER Task 5 if your tooling fails on the missing import. If `npm run build` fails solely with "Usage.svelte not found", that's expected; proceed to Task 5 and build there.

- [ ] **Step 5: Commit** (after Task 5 build is green, commit together — see Task 5 Step 5)

---

## Task 5: `Usage.svelte` screen

**Files:**
- Create: `ui/src/routes/Usage.svelte`

- [ ] **Step 1: Create the component**

Create `ui/src/routes/Usage.svelte`:

```svelte
<script lang="ts">
  import { api, type UsageReport, type UsageNode } from "../ipc";
  import { humanBytes } from "../lib/format";
  import Icon from "../lib/Icon.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { onDestroy, onMount } from "svelte";
  import { fly, slide } from "svelte/transition";

  let drives: string[] = [];
  let drive = "C:\\";
  let path = "C:\\";
  let report: UsageReport | null = null;
  let selected = new Set<string>();
  let scanning = false;
  let working = false;
  let status = "";
  let statusKind: "ok" | "warn" | "" = "";
  let progressLabel = "";

  let unlisten: (() => void) | undefined;
  listen<string>("analyze://progress", (e) => {
    progressLabel = e.payload;
  }).then((fn) => (unlisten = fn));
  onDestroy(() => unlisten?.());

  onMount(async () => {
    try {
      drives = await api.listDrives();
      if (drives.length && !drives.includes(drive)) {
        drive = drives[0];
        path = drives[0];
      }
    } catch {
      /* keep default C:\ */
    }
  });

  function crumbs(p: string): { label: string; path: string }[] {
    const parts = p.replace(/\\+$/, "").split("\\").filter(Boolean);
    const out: { label: string; path: string }[] = [];
    let acc = "";
    parts.forEach((seg, idx) => {
      acc = idx === 0 ? seg + "\\" : acc.replace(/\\?$/, "\\") + seg;
      out.push({ label: seg, path: acc });
    });
    return out;
  }

  async function scan(target = path) {
    path = target;
    scanning = true;
    status = "";
    statusKind = "";
    progressLabel = "";
    selected = new Set();
    try {
      report = await api.analyzeDisk(target);
    } catch (e) {
      status = `Ошибка анализа: ${e}`;
      statusKind = "warn";
      report = null;
    } finally {
      scanning = false;
    }
  }

  function openNode(n: UsageNode) {
    if (n.is_dir && n.path) scan(n.path);
    else if (!n.protected && n.path) toggle(n.path);
  }

  function toggle(p: string) {
    selected.has(p) ? selected.delete(p) : selected.add(p);
    selected = selected;
  }

  function volumeRoot(p: string): string {
    if (p.startsWith("\\\\")) {
      const third = p.indexOf("\\", 2);
      const fourth = third >= 0 ? p.indexOf("\\", third + 1) : -1;
      return fourth >= 0 ? p.slice(0, fourth + 1) : p;
    }
    return p.slice(0, 2) + "\\";
  }

  async function quarantine() {
    const paths = [...selected];
    if (!paths.length) return;
    working = true;
    status = "";
    statusKind = "";
    const byVol = new Map<string, string[]>();
    for (const p of paths) {
      const v = volumeRoot(p);
      byVol.set(v, [...(byVol.get(v) ?? []), p]);
    }
    let freed = 0;
    let skipped = 0;
    const errors: string[] = [];
    for (const [v, ps] of byVol) {
      try {
        const r = await api.quarantineItems("Анализ диска", v, ps);
        freed += r.freed;
        skipped += r.skipped.length;
        if (!r.ok) errors.push(`${v} — ${r.message}`);
      } catch (e) {
        errors.push(`${v} — ${e}`);
      }
    }
    const parts = [`Освобождено ${humanBytes(freed)}`];
    if (skipped) parts.push(`пропущено ${skipped} (заняты или недоступны)`);
    if (errors.length) parts.push(`ошибки: ${errors.join("; ")}`);
    else if (!skipped) parts[0] = `Освобождено ${humanBytes(freed)} — всё в карантине, можно вернуть`;
    status = parts.join(" · ");
    statusKind = errors.length ? "warn" : "ok";
    working = false;
    await scan(path);
  }

  $: tiles = report ? report.tree.children : [];
  $: maxTile = tiles.reduce((m, t) => Math.max(m, t.size), 1);
  $: sizeOf = (() => {
    const m = new Map<string, number>();
    if (report) {
      for (const n of report.tree.children) if (n.path) m.set(n.path, n.size);
      for (const f of report.largest) m.set(f.path, f.size);
    }
    return m;
  })();
  $: selTotal = [...selected].reduce((s, p) => s + (sizeOf.get(p) ?? 0), 0);
  $: used = report ? Math.max(0, report.volume_total - report.free) : 0;
  $: usedPct = report && report.volume_total ? Math.round((used / report.volume_total) * 100) : 0;
</script>

<header class="bar">
  <span class="hicon"><Icon name="analyze" size={20} /></span>
  <div>
    <h2>Анализ диска</h2>
    <div class="sub faint mono">
      {#if scanning}сканирую…{:else if report}{humanBytes(report.total)} в {report.tree.children.length} элементах{:else}выбери диск и нажми «Сканировать»{/if}
    </div>
  </div>
  <span class="grow"></span>

  <select class="drivesel" bind:value={drive} on:change={() => scan(drive)} disabled={scanning || working}>
    {#each drives as d}<option value={d}>{d}</option>{/each}
    {#if !drives.length}<option value={drive}>{drive}</option>{/if}
  </select>
  <button class="btn btn--ghost btn--sm" on:click={() => scan(drive)} disabled={scanning || working}>
    {#if scanning}<span class="spinner"></span>{:else}<Icon name="refresh" size={16} />{/if}
    Сканировать
  </button>
</header>

{#if report}
  <div class="diskbar" transition:fly={{ y: -6, duration: 180 }}>
    <div class="track"><div class="fill" style="width:{usedPct}%"></div></div>
    <span class="faint mono">занято {humanBytes(used)} из {humanBytes(report.volume_total)} · свободно {humanBytes(report.free)}</span>
  </div>
{/if}

{#if scanning || working}
  <div class="liveband" transition:slide={{ duration: 160 }}>
    <div class="progress"></div>
    <div class="livetext faint mono">
      {#if working}переношу в карантин…{:else if progressLabel}сканирую: {progressLabel}{:else}сканирую…{/if}
    </div>
  </div>
{/if}

{#if status}
  <p class="status" class:warn={statusKind === "warn"} class:ok={statusKind === "ok"} transition:fly={{ y: -4, duration: 160 }}>
    {#if statusKind === "ok"}<Icon name="check" size={15} />{:else if statusKind === "warn"}<Icon name="alert" size={15} />{/if}
    <span>{status}</span>
  </p>
{/if}

{#if report}
  <nav class="crumbs">
    {#each crumbs(path) as c, i (c.path)}
      {#if i > 0}<span class="crsep"><Icon name="chevron" size={12} /></span>{/if}
      <button class="crumb" class:cur={c.path.replace(/\\+$/, "") === path.replace(/\\+$/, "")} on:click={() => scan(c.path)}>{c.label}</button>
    {/each}
    <span class="grow"></span>
    <span class="faint crumbhint">клик по папке — внутрь</span>
  </nav>

  <div class="treemap">
    {#each tiles as t (t.path + t.name)}
      <button
        class="tile"
        class:isdir={t.is_dir}
        class:prot={t.protected}
        class:sel={selected.has(t.path)}
        style="flex-grow:{Math.max(1, Math.round((t.size / maxTile) * 100))}"
        on:click={() => openNode(t)}
        disabled={t.protected && !t.is_dir}
        title={t.path || t.name}
      >
        <span class="tname">
          {#if t.protected}<Icon name="quarantine" size={13} />{:else if t.is_dir}<Icon name="folder" size={13} />{/if}
          <span class="tlabel">{t.name}</span>
        </span>
        <span class="tsize mono">{humanBytes(t.size)}</span>
      </button>
    {/each}
  </div>

  <div class="sechead">
    <span class="eyebrow">самые крупные файлы</span>
    <span class="grow"></span>
    {#if selected.size}
      <span class="faint mono">выбрано {selected.size} · <span class="accent">{humanBytes(selTotal)}</span></span>
    {/if}
  </div>

  <div class="files">
    {#each report.largest as f (f.path)}
      <label class="listrow" class:is-selected={selected.has(f.path)}>
        {#if f.protected}
          <span class="lockcell"><Icon name="quarantine" size={16} /></span>
        {:else}
          <input class="check" type="checkbox" checked={selected.has(f.path)} on:change={() => toggle(f.path)} />
        {/if}
        <span class="path">{f.path}</span>
        <span class="size mono">{humanBytes(f.size)}</span>
      </label>
    {/each}
    {#if !report.largest.length}
      <div class="empty"><div class="ring"><Icon name="analyze" size={28} /></div><div>Крупных файлов не найдено</div></div>
    {/if}
  </div>

  <div class="footer">
    <span class="faint">{#if selected.size}выбрано {selected.size} · {humanBytes(selTotal)}{:else}отметь файлы или плитки для переноса{/if}</span>
    <span class="grow"></span>
    <button class="btn btn--accent" on:click={quarantine} disabled={!selected.size || scanning || working}>
      {#if working}<span class="spinner"></span>{:else}<Icon name="quarantine" size={16} />{/if}
      В карантин · обратимо
    </button>
  </div>
{:else if !scanning}
  <div class="empty">
    <div class="ring"><Icon name="analyze" size={28} /></div>
    <div>Выбери диск и нажми «Сканировать»</div>
    <div class="faint" style="font-size:12.5px">Покажу, что занимает место, и дам убрать лишнее в карантин</div>
  </div>
{/if}

<style>
  .hicon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 11px;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--acc);
    flex: 0 0 auto;
  }
  .sub {
    font-size: 12px;
    margin-top: 2px;
  }
  .drivesel {
    font-family: var(--font-mono);
    font-size: 12.5px;
    background: #00000033;
    color: var(--text);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-sm);
    padding: 7px 9px;
    cursor: pointer;
  }
  .drivesel:focus {
    outline: none;
    border-color: var(--acc);
  }
  .diskbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 14px;
    font-size: 12px;
  }
  .track {
    flex: 1;
    height: 8px;
    border-radius: 999px;
    background: #20232b;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--acc);
    transition: width 0.5s var(--ease, ease);
  }
  .liveband {
    margin-top: 14px;
  }
  .livetext {
    font-size: 12px;
    margin-top: 8px;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 13px;
    border-radius: var(--r-sm);
    padding: 10px 13px;
    margin: 14px 0 0;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--text-dim);
  }
  .status.ok {
    color: var(--acc);
    background: var(--acc-soft);
    border-color: rgba(198, 242, 78, 0.3);
  }
  .status.warn {
    color: var(--danger);
    background: var(--danger-soft);
    border-color: rgba(255, 111, 97, 0.35);
  }
  .crumbs {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 16px;
    flex-wrap: wrap;
  }
  .crsep {
    color: var(--text-faint);
    display: grid;
    place-items: center;
  }
  .crumb {
    border: 0;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    font-family: var(--font-display);
    font-size: 13px;
    padding: 3px 7px;
    border-radius: 6px;
  }
  .crumb:hover {
    color: var(--text);
    background: #ffffff0a;
  }
  .crumb.cur {
    color: var(--acc);
  }
  .crumbhint {
    font-size: 11.5px;
  }
  .treemap {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
  .tile {
    flex: 1 1 92px;
    min-width: 92px;
    min-height: 66px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 6px;
    text-align: left;
    padding: 9px 11px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--text);
    cursor: default;
    overflow: hidden;
    transition:
      transform 0.12s ease,
      border-color 0.15s ease,
      background 0.15s ease;
  }
  .tile.isdir {
    cursor: pointer;
  }
  .tile.isdir:hover {
    transform: translateY(-1px);
    border-color: var(--line-strong);
    background: #272b34;
  }
  .tile.sel {
    background: var(--acc-soft);
    border-color: rgba(198, 242, 78, 0.4);
    color: var(--acc);
  }
  .tile.prot {
    background: #15161b;
    border-style: dashed;
    color: var(--text-faint);
    cursor: not-allowed;
  }
  .tname {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-display);
    font-size: 12.5px;
    min-width: 0;
  }
  .tlabel {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tsize {
    font-size: 11.5px;
    color: var(--text-dim);
  }
  .tile.sel .tsize {
    color: var(--acc);
  }
  .sechead {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 22px 0 6px;
  }
  .accent {
    color: var(--acc);
  }
  .files {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .lockcell {
    width: 19px;
    display: grid;
    place-items: center;
    color: var(--text-faint);
    flex: 0 0 auto;
  }
  .path {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    word-break: break-all;
    color: var(--text-dim);
  }
  .size {
    font-size: 12.5px;
    white-space: nowrap;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 18px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
    font-size: 12.5px;
  }
</style>
```

- [ ] **Step 2: Build the UI**

Run: `npm --prefix ui run build`
Expected: builds with no TS/Svelte errors.

- [ ] **Step 3: Lint the whole workspace once**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Manual smoke test**

Run (from an elevated terminal, per README):
```
cd crates/svalka-app
../../ui/node_modules/.bin/tauri dev
```
Verify:
- The «Анализ диска» tab appears between «Программы» and «Карантин».
- Picking a drive + «Сканировать» fills the treemap and the disk bar; progress shows folder names.
- Clicking a folder tile drills in; breadcrumb navigates back out.
- `hiberfil.sys`/`pagefile.sys`/System32 show as protected (dashed/lock) and can't be selected.
- Selecting a large file → «В карантин» → toast shows freed bytes; the item appears in the Карантин screen and restores correctly.

- [ ] **Step 5: Commit**

```bash
git add ui/src/routes/Usage.svelte ui/src/App.svelte ui/src/lib/Icon.svelte
git commit -m "feat(ui): Анализ диска screen (treemap + largest files, quarantine-backed)" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Docs

**Files:**
- Modify: `README.md` (mention the new screen)

- [ ] **Step 1: Update the screens line**

In `README.md`, update the UI screens description (around the `ui` bullet under Architecture) to include the fifth screen:

```
- **`ui`** renders five screens: **Очистка** (clean), **Программы** (programs),
  **Анализ диска** (disk usage treemap + largest files), **Карантин**
  (quarantine), **Настройки** (settings).
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: mention the Анализ диска screen" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- Treemap + drill-down → Task 5 (tiles + breadcrumb + `openNode`). ✓
- Largest files → Task 1 (`TopN`/`largest`) + Task 5 list. ✓
- Quarantine reuse → Task 5 `quarantine()` calls existing `quarantineItems`. ✓
- Deny-list / protected → Task 1 `mark_protected` + Task 5 disabled selection. ✓
- Free/total header → Task 1 `disk_space` + Task 5 disk bar. ✓
- Progress events → Task 2 `analyze://progress` + Task 5 listener. ✓
- Depth/top-K/«остальное» bounding → Task 1 `scan_node`/`fold_children` + tests. ✓
- Reparse-point safety, unreadable dirs, missing root → Task 1 impl + tests. ✓
- Drive picker (`list_drives`) → Task 2 + Task 5. ✓
- Tests (8 spec items) → Task 1 has 7 tests covering total, sort, top-N, depth, fold, protected, missing root; reparse handling is covered by impl (symlink leaf) and not unit-tested (Windows symlink creation needs privilege — documented gap, matches the spec's "if creatable"). ✓

**Placeholder scan:** no TBD/TODO; every code step has complete code; every command has expected output. ✓

**Type consistency:** `analyze(root, Limits, F: Fn(&str))` matches the Task 2 closure `|name| ...`; `UsageReport`/`UsageNode`/`LargeFile` field names identical across `usage.rs` (Task 1), `ipc.ts` (Task 3), and `Usage.svelte` (Task 5); `analyzeDisk`/`listDrives` names match commands `analyze_disk`/`list_drives`; `quarantineItems(source, volumeRoot, paths)` signature matches existing `ipc.ts`. ✓

One known gap (intentional, per spec §10): reparse-point non-recursion isn't unit-tested because creating a junction/symlink in tests needs elevation on Windows; it's covered by the `symlink_metadata` guard in `scan_node`.
