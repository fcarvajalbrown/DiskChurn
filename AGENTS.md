# DiskChurn — Agent Instructions

## Project

Native Windows disk analyzer in Rust. Reads MFT file metadata, classifies folders by churn behavior, samples file entropy, and renders a treemap + folder list via egui.

Author: Felipe Carvajal Brown

## Build and Run

```
cargo build                  # debug
cargo build --release        # release (CI mode)
cargo check                  # fast type/borrow check without linking
cargo run                    # launch the GUI (requires MSVC toolchain)
```

Requires Windows with MSVC C++ workload installed. MFT scan requires Administrator; the app falls back to walkdir silently.

## File Map

| File | Role |
|---|---|
| `src/types.rs` | Core types: `FileNode`, `FolderStats`, `ChurnClass`, `EntropyClass`, `DiskSnapshot` |
| `src/scanner.rs` | MFT walk via `DeviceIoControl` + walkdir fallback; emits `FileNode` batches over mpsc |
| `src/classifier.rs` | `ChurnClass` assignment + linear regression growth projection |
| `src/entropy.rs` | 64 KB Shannon entropy sampler, `EntropyClass` assignment |
| `src/treemap.rs` | Squarify layout algorithm, egui painter rendering |
| `src/app.rs` | egui `App` state machine, panels, drive dropdown, filters |
| `src/main.rs` | `eframe::run_native` bootstrap |
| `build.rs` | Sets `/SUBSYSTEM:WINDOWS` to suppress console window |

## Key Types (src/types.rs)

```rust
pub enum ChurnClass { Cold, Hot, Volatile }
pub enum EntropyClass { Compressible, Mixed, Dense }

pub struct FileNode {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub entropy: Option<f32>, // 0.0–8.0 scale, None until entropy pass
    pub ntfs_compressed: bool,
}

pub struct FolderStats {
    pub path: PathBuf,
    pub total_size: u64,
    pub file_count: u64,
    pub churn: ChurnClass,
    pub entropy_class: EntropyClass,
    pub days_until_full: Option<f32>,   // Some only for Hot
    pub reclaimable_bytes: Option<u64>, // Some only for Compressible/Mixed
    pub children: Vec<FolderStats>,
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

## Architecture

- Scanner runs in `std::thread::spawn`, sends `FileNode` batches over `std::sync::mpsc`
- App holds `Arc<Mutex<DiskSnapshot>>` updated as batches arrive
- Treemap layout recomputed only on snapshot change, not every frame
- Entropy sampler runs after MFT metadata pass; skips files where `ntfs_compressed == true`
- `build.rs` suppresses console window via `/SUBSYSTEM:WINDOWS`

## Churn Classification Rules

- **Cold** — files mostly created >90 days ago, rarely modified; large untouched archives
- **Hot** — high ratio of recently modified/created files, size trending up; shows projected days until full via linear regression
- **Volatile** — high file count churn, small average file size, short file lifetimes; temp/cache/log dirs

## Entropy Thresholds

- **< 6.0 bits/byte** → `Compressible`
- **6.0–7.2** → `Mixed`
- **> 7.2 bits/byte** → `Dense`

## Commits

All commits must use conventional commits. Keep messages short and single-line.
Valid prefixes: `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`, `ci:`, `test:`.

## Coding Rules

- One-line comments only, informal tone
- No multi-line or block comments
- No emojis in code, docs, or commits
- Root-cause fixes only — no workarounds
- No error handling or validation for scenarios that cannot happen
- No abstractions beyond what the current task requires

## Current State (v0.1)

Completed: `types.rs`, `scanner.rs`, `classifier.rs`
Stubs (need implementation): `entropy.rs`, `treemap.rs`, `app.rs`, `main.rs`

Next up: implement `src/entropy.rs` — 64 KB Shannon entropy sampler returning `EntropyClass` per file.
