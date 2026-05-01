# DiskChurn — Claude Context

## Project
Native Windows disk analyzer in Rust. Scans MFT for file metadata, classifies folders by churn behavior, samples file entropy, and renders a treemap + folder list via egui.

Author: Felipe Carvajal Brown

## Stack
- Language: Rust (edition 2021)
- GUI: egui + eframe 0.27
- Disk scan: windows-rs (MFT via DeviceIoControl), walkdir fallback
- Charts: egui_plot

## File Map
- `src/types.rs` — core types: FileNode, FolderStats, ChurnClass, EntropyClass, DiskSnapshot
- `src/scanner.rs` — MFT walk + walkdir fallback, emits FileNode batches over mpsc channel
- `src/classifier.rs` — ChurnClass assignment + linear regression growth projection
- `src/entropy.rs` — 64 KB Shannon entropy sampler, EntropyClass assignment
- `src/treemap.rs` — Squarify layout, egui painter rendering
- `src/app.rs` — egui App state machine, panels, drive dropdown, filters
- `src/main.rs` — eframe::run_native bootstrap

## Commits
All commits must use conventional commits. Keep messages short and single-line.
Valid prefixes: `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`, `ci:`, `test:`.

## Coding Rules
- One-line comments only, informal tone
- No multi-line or block comments
- No emojis in code, docs, or commits
- Root-cause fixes only — no workarounds

## Architecture Notes
- Scanner runs in std::thread::spawn, sends batches over mpsc
- App holds Arc<Mutex<DiskSnapshot>> updated as batches arrive
- Treemap layout recomputed only on snapshot change, not every frame
- MFT scan requires Administrator; fallback to walkdir is automatic and silent
- build.rs sets /SUBSYSTEM:WINDOWS to suppress console window

## Current Version
v0.1 in progress — types, scanner, classifier done; entropy, treemap, app, main are stubs

## Session Notes
- Cargo.lock is tracked (binary crate)
- .gitignore excludes /target only
- CI runs on windows-latest, release build
- MSVC C++ workload was missing — user reinstalled; cargo check should work after restart
- Next: implement src/entropy.rs (64 KB Shannon entropy sampler)
- .claude/settings.json has Stop hook reminding to update CLAUDE.md
- .claude/skills/hooks.md has hooks reference doc
