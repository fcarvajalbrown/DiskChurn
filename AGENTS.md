# DiskChurn — Agent Instructions

## Project

Native Windows disk analyzer in Rust. Reads MFT file metadata, classifies folders by churn behavior, samples file entropy, renders a treemap + folder list via egui, and tracks scan history with delta comparison.

Author: Felipe Carvajal Brown

## Build and Run

```
cargo build                  # debug
cargo build --release        # release (CI mode)
cargo check                  # fast type/borrow check without linking
cargo test                   # run all 40 tests
cargo run                    # launch the GUI (requires MSVC toolchain)
```

Requires Windows with MSVC C++ workload installed. MFT scan requires Administrator; the app falls back to walkdir silently.

## File Map

| File | Role |
|---|---|
| `src/lib.rs` | Crate root; declares all modules as `pub mod` for integration test access |
| `src/types.rs` | Core types: `FileNode`, `FolderStats`, `ChurnClass`, `EntropyClass`, `DiskSnapshot` |
| `src/scanner.rs` | MFT walk via `DeviceIoControl` + walkdir fallback; emits `FileNode` batches over mpsc |
| `src/classifier.rs` | `ChurnClass` assignment + linear regression growth projection |
| `src/entropy.rs` | 64 KB Shannon entropy sampler, `EntropyClass` assignment |
| `src/treemap.rs` | Squarify layout algorithm, egui painter rendering, delta paint variant |
| `src/delta.rs` | `FolderDelta` type; `compute()` diffs two snapshots; `apply_min_filter()` is display-layer guard |
| `src/history.rs` | bincode snapshot persistence to `%APPDATA%\DiskChurn\snapshots\`; max 10 per drive |
| `src/app.rs` | egui `App` state machine, panels, drive dropdown, compare toolbar, filters |
| `src/main.rs` | `eframe::run_native` bootstrap |
| `build.rs` | Sets `/SUBSYSTEM:WINDOWS` to suppress console window |
| `tests/classifier.rs` | Integration tests: aggregation, Cold/Hot/Volatile classification |
| `tests/delta.rs` | Integration tests: compute axioms, apply_min_filter |
| `tests/entropy.rs` | Integration tests: Shannon math, boundary thresholds |
| `tests/treemap.rs` | Integration tests: layout edge cases, area conservation, fmt_delta |
| `tests/history.rs` | Integration tests: bincode round-trip, field preservation |

## Key Types (src/types.rs)

```rust
pub enum ChurnClass { Cold, Hot, Volatile }
pub enum EntropyClass { Compressible, Mixed, Dense }

pub struct FileNode {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: SystemTime,
    pub entropy: Option<f32>, // 0.0–8.0 scale, None until entropy pass
}

pub struct FolderStats {
    pub path: PathBuf,
    pub total_size: u64,
    pub file_count: u64,
    pub churn: ChurnClass,
    pub entropy_class: EntropyClass,
    pub days_until_full: Option<f32>, // Some only for Hot
}

pub struct DiskSnapshot {
    pub drive: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub files: Vec<FileNode>,
    pub folders: Vec<FolderStats>,
    pub scan_complete: bool,
    pub files_scanned: u64,
}
```

## Key Types (src/delta.rs and src/history.rs)

```rust
pub enum DeltaKind { New, Deleted, Grown, Shrunk, Unchanged }

pub struct FolderDelta {
    pub path: PathBuf,
    pub old_size: u64,
    pub new_size: u64,
    pub delta: i64,
    pub kind: DeltaKind,
}

pub struct Snapshot {
    pub drive: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub files_scanned: u64,
    pub folders: Vec<FolderStats>,
    pub scanned_at: u64, // unix seconds
}
```

## Architecture

- Scanner runs in `std::thread::spawn`, sends `FileNode` batches over `std::sync::mpsc`; rayon parallelizes MFT entry processing
- App holds `Arc<Mutex<DiskSnapshot>>` updated as batches arrive
- Treemap layout recomputed only on snapshot change, not every frame
- Entropy pass runs in the parallel scan pass alongside file stat collection
- `history::save()` called after each completed scan; `prune()` keeps max 10 snapshots per drive
- `delta::compute()` never filters; `apply_min_filter()` is the display-layer materiality guard
- Crate split into lib (`src/lib.rs`) + bin (`src/main.rs`) so `tests/` can import the public API
- `build.rs` suppresses console window via `/SUBSYSTEM:WINDOWS`

## Churn Classification Rules

- **Cold** — files mostly untouched >90 days; typical for archives and installers
- **Hot** — >30% of files modified in the last 30 days; shows projected days until disk full via linear regression
- **Volatile** — >=20 files with >60% modified in last 7 days and avg file size <10 MB; temp/cache/log dirs

## Entropy Thresholds

- **< 6.0 bits/byte** → `Compressible`
- **6.0–7.2** → `Mixed`
- **> 7.2 bits/byte** → `Dense`

## Delta View

- Compare toolbar lets user pick any saved snapshot as baseline
- Log-scale slider sets materiality threshold (0–10 GB); default 0 shows everything
- Four kind toggles: New / Grown / Shrunk / Deleted
- Sidebar caps at 500 entries with overflow notice
- Sub-pixel pre-filter drops items whose pixel area would be < 1 px before calling layout

## Commits

All commits must use conventional commits. Keep messages short and single-line.
Valid prefixes: `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`, `ci:`, `test:`.
NEVER add a Co-Authored-By trailer or any other co-authorship line to commits.

## Coding Rules

- One-line comments only, informal tone
- No multi-line or block comments
- No emojis in code, docs, or commits
- Root-cause fixes only — no workarounds
- No error handling or validation for scenarios that cannot happen
- No abstractions beyond what the current task requires

## Current State (v0.1)

All modules implemented, 40 tests passing, clean build with zero warnings.
