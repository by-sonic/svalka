# Svalka

A safe, transparent Windows cleaner. Svalka scans real junk (system / browser /
developer caches), uninstalls programs in batches while showing their **real**
on-disk sizes, hunts uninstall leftovers — and makes **every deletion
reversible** by moving things into a per-volume quarantine instead of deleting
them outright.

> Svalka (Сва́лка) — "the dump". Nothing is destroyed; it just gets moved to the
> dump, where you can fish it back out.

## Why it's safe

- **Nothing is deleted — it's quarantined.** Junk is moved (atomic, instant,
  same-volume `rename`) into `<volume>\.svalka-quarantine\<id>\`, recorded in a
  JSON manifest, and can be restored to its exact original path. Each file gets
  its own indexed subfolder, so two items with the same name (e.g. Chrome's and
  Edge's `Cache`) never collide.
- **Hard deny-list.** Even if a scanner returns one, protected locations
  (`System32`, `WinSxS`, browser `Cookies`/`Login Data`, `hiberfil.sys`, …) are
  dropped. Paths are canonicalized first, so 8.3 short-names and junctions can't
  sneak past the check.
- **Resilient batches, never orphaned.** Items are moved independently: a
  locked / in-use file (an open browser holding its cache is the normal case)
  is skipped and reported, never fatal — one bad item can't abort the whole
  cleanup. The manifest is still written atomically: if recording the batch
  fails, the moves are rolled back so nothing is ever left moved-but-unrecorded
  (and therefore unrestorable).
- **Restore never clobbers.** If something already occupies an original path,
  the restore aborts instead of overwriting.
- **Auto-purge.** Quarantine entries older than 30 days are removed on launch.
- **Registry safety.** Registry keys are exported to a `.reg` file (via the
  fully-qualified `reg.exe`) before any change, and re-imported on restore.
- **No silent failures.** Uninstall and quarantine outcomes carry messages;
  failures are surfaced in the UI, never swallowed.

## Architecture

```
Svalka/
  crates/
    svalka-core/   # all logic, headless, fully unit-tested (no Tauri imports)
    svalka-app/    # thin Tauri 2 wrapper: IPC commands + state over the core
  ui/              # Svelte 5 + Vite frontend (five screens)
```

- **`svalka-core`** is a plain Rust library with no GUI dependency, so it
  compiles and tests on any host. Modules: `model`, `fsafe` (atomic move +
  same-volume + in-use detection), `sizer` (parallel sizing via `jwalk`),
  `usage` (bounded disk-usage tree + largest files),
  `quarantine` (move / restore / purge + manifest), `regbackup` (reg.exe
  export/import), `scanner` (categories + deny-list), `programs` (installed
  programs across registry hives with real folder sizes), `uninstaller`,
  `leftovers`, `paths` (env-injected well-known roots).
- **`svalka-app`** maps Tauri IPC commands/events onto the core and requests
  administrator rights at launch (needed to clean system folders).
- **`ui`** renders five screens: **Очистка** (clean), **Программы** (programs),
  **Анализ диска** (disk usage treemap + largest files), **Карантин**
  (quarantine), **Настройки** (settings).

## Prerequisites

- Rust (stable) + cargo
- Node.js + npm
- Windows (the cleaning logic and registry features are Windows-specific)

## Develop

```sh
# install frontend deps
npm --prefix ui install

# run the app in dev (starts Vite, opens the Tauri window; prompts for UAC)
# NB: the app's manifest requests administrator rights, so launch this from an
# elevated terminal — otherwise the UAC-elevated window can't talk back to the
# dev tooling.
cd crates/svalka-app
../../ui/node_modules/.bin/tauri dev
```

## Test

```sh
cargo test -p svalka-core        # core engine: unit + integration tests
cargo clippy -p svalka-core --all-targets -- -D warnings
npm --prefix ui run test         # frontend unit tests (vitest)
```

## Build a release installer

```sh
cd crates/svalka-app
../../ui/node_modules/.bin/tauri build   # produces an NSIS installer under target/release/bundle
```

## Milestones

- **M1 — Core engine** (`cargo test -p svalka-core` green): a headless,
  fully-tested library that scans, sizes, quarantines, restores, and reads
  installed programs with real sizes.
- **M2 — App + UI**: the Tauri app and the five Svelte screens wired to the core.

## Known v1 limitations

- **Settings → auto-purge days** is shown but not yet persisted; the launch
  purge is hardcoded to 30 days. Wiring it up is a small follow-up
  (`get_settings`/`set_settings` backed by `settings.json` in the app data dir).
- **Recycle Bin** category is modeled but emptied via the Windows Shell API in a
  later iteration, not via path scanning.
- **Registry-key quarantine** is modeled end-to-end (`RegBackup`, export/import
  on restore) but no screen quarantines a registry key yet — file leftovers are
  covered; reg-key quarantine reuses the same machinery.
- Icons are placeholder art.

## License

Personal project — no license specified yet.
