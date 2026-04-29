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

## Coding Rules
- One-line comments only, informal tone
- No multi-line or block comments
- No emojis in code, docs, or commits
- Commit messages: conventional commits, single short lines
- Root-cause fixes only — no workarounds

## Architecture Notes
- Scanner runs in std::thread::spawn, sends batches over mpsc
- App holds Arc<Mutex<DiskSnapshot>> updated as batches arrive
- Treemap layout recomputed only on snapshot change, not every frame
- MFT scan requires Administrator; fallback to walkdir is automatic and silent
- build.rs sets /SUBSYSTEM:WINDOWS to suppress console window

## Current Version
v0.1 — initial file structure, types, scanner, classifier in place
