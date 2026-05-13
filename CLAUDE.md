# DiskChurn — Claude Context

## Project
Native Windows disk analyzer in Rust. Scans MFT for file metadata, classifies folders by churn behavior, samples file entropy, renders a treemap + folder list via egui, and tracks scan history with delta comparison.

Author: Felipe Carvajal Brown

## Stack
- Language: Rust (edition 2021)
- GUI: egui + eframe 0.27
- Disk scan: windows-rs (MFT via DeviceIoControl), walkdir fallback
- Serialization: bincode + serde
- Parallelism: rayon

## File Map
- `src/lib.rs` — crate root; re-exports all modules as pub mod
- `src/types.rs` — core types: FileNode, FolderStats, ChurnClass, EntropyClass, DiskSnapshot
- `src/scanner.rs` — MFT walk + walkdir fallback, emits FileNode batches over mpsc channel
- `src/classifier.rs` — ChurnClass assignment + linear regression growth projection
- `src/entropy.rs` — 64 KB Shannon entropy sampler, EntropyClass assignment
- `src/treemap.rs` — Squarify layout, egui painter rendering, delta paint variant
- `src/delta.rs` — FolderDelta type, compute() and apply_min_filter() for snapshot diffs
- `src/history.rs` — bincode snapshot persistence to %APPDATA%\DiskChurn\snapshots\
- `src/app.rs` — egui App state machine, panels, drive dropdown, compare toolbar, filters
- `src/main.rs` — eframe::run_native bootstrap

## Commits
All commits must use conventional commits. Keep messages short and single-line.
Valid prefixes: `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`, `ci:`, `test:`.
NEVER add a Co-Authored-By trailer or any other co-authorship line to commits.

## Coding Rules
- One-line comments only, informal tone
- No multi-line or block comments
- No emojis in code, docs, or commits
- Root-cause fixes only — no workarounds
- Use only conventional commits

## Architecture Notes
- Scanner runs in std::thread::spawn, sends batches over mpsc; rayon parallelizes MFT entry processing
- App holds Arc<Mutex<DiskSnapshot>> updated as batches arrive
- Treemap layout recomputed only on snapshot change, not every frame
- MFT scan requires Administrator; fallback to walkdir is automatic and silent
- build.rs sets /SUBSYSTEM:WINDOWS to suppress console window
- Entropy pass runs in rayon parallel scan pass alongside file stat collection
- history::save() called after each completed scan; keeps max 10 snapshots per drive
- delta::compute() emits all changes including Unchanged; apply_min_filter() is the display-layer guard
- Crate is split into lib (src/lib.rs) + bin (src/main.rs) so tests/ integration tests can import pub API

## Current Version
v0.1 complete — all modules implemented, 40 tests passing, zero warnings, clean build

## Session Notes
- Cargo.lock is tracked (binary crate)
- .gitignore excludes /target and /.claude
- CI runs on windows-latest, release build
- .claude/settings.json has Stop hook reminding to update CLAUDE.md
- .claude/skills/hooks.md has hooks reference doc
- FileNode has no created/ntfs_compressed (removed as dead); FolderStats has no reclaimable_bytes/children
- Integration tests live in tests/; private fns (e.g. drive_key) keep inline #[cfg(test)] blocks
