# DiskChurn — NTFS Disk Space Analyzer for Windows

<p align="center">
  <img src="https://img.shields.io/badge/platform-Windows%2010%2B-0078D4?logo=windows&logoColor=white" alt="Platform: Windows 10+">
  <img src="https://img.shields.io/badge/language-Rust%202021-CE422B?logo=rust&logoColor=white" alt="Language: Rust 2021">
  <img src="https://img.shields.io/badge/GUI-egui%200.27-5C6AC4" alt="GUI: egui 0.27">
  <img src="https://img.shields.io/badge/tests-40%20passing-brightgreen" alt="40 tests passing">
  <img src="https://img.shields.io/badge/version-0.1.0-informational" alt="Version 0.1.0">
  <img src="https://img.shields.io/badge/license-GPL--3.0-blue" alt="License: GPL-3.0">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/binary-single%20exe-success" alt="Single exe">
  <img src="https://img.shields.io/badge/admin-optional-lightgrey" alt="Admin optional">
  <img src="https://img.shields.io/badge/NTFS-MFT%20direct-blueviolet" alt="NTFS MFT direct">
  <img src="https://img.shields.io/badge/no%20runtime%20deps-yes-brightgreen" alt="No runtime dependencies">
</p>

> Fast, single-scan Windows disk analyzer in Rust. Reads the NTFS Master File Table directly to classify folders by growth behavior, project when your disk will fill up, estimate compression savings via Shannon entropy, and compare snapshots over time — all in one native egui window.

---

## Why DiskChurn?

Tools like **WinDirStat**, **WizTree**, and **TreeSize** show what is large *right now*. They cannot tell you what is **growing**, **when the disk will fill up**, or **whether a folder is actually worth compressing**. DiskChurn answers all three in a single scan pass.

| Question | WinDirStat / WizTree | DiskChurn |
|---|:---:|:---:|
| What folders are large? | Yes | Yes |
| Which folders are actively growing? | No | **Yes** |
| Days until disk full? | No | **Yes** |
| Is this folder compressible? | No | **Yes** |
| Change tracking across time? | No | **Yes** |

---

## Features

- **Churn classification** — reads NTFS MFT timestamps to label every folder as `Cold` (untouched), `Hot` (actively growing), or `Volatile` (high file turnover). No multi-day snapshot required.
- **Growth projection** — for Hot folders, fits a linear regression on modification timestamps and projects days until disk full.
- **Entropy analysis** — samples the first 64 KB of each file, computes Shannon entropy, and estimates how much space compression would actually recover. Automatically skips already-dense files (ZIPs, JPEGs, MP4s, etc.).
- **Interactive treemap** — folders rendered as area-proportional rectangles, color-coded by churn class. Delta paint mode highlights growth and shrinkage between snapshots.
- **Snapshot history** — saves up to 10 snapshots per drive to `%APPDATA%\DiskChurn\snapshots\` automatically after each scan.
- **Delta comparison** — compare any two historical snapshots to see which folders grew, shrank, appeared, or disappeared, with a user-controlled materiality threshold (0–10 GB log-scale slider).
- **MFT-direct speed** — reads the NTFS Master File Table via `DeviceIoControl`; falls back to `walkdir` automatically when running without Administrator or on non-NTFS volumes.
- **Zero dependencies at runtime** — single `.exe`, no installer, no .NET or redistributable required.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust (edition 2021) |
| GUI | egui + eframe 0.27 |
| Disk scan | windows-rs — MFT via `DeviceIoControl` |
| Fallback scan | walkdir (non-admin or non-NTFS) |
| Parallelism | rayon |
| Serialization | bincode + serde |

---

## Building

**Requirements:** Rust stable + Windows with the MSVC C++ Build Tools workload.

```powershell
cargo build --release
```

Binary lands at `target\release\diskchurn.exe`. No installer. No runtime dependencies.

> Run as Administrator to enable MFT-direct scanning. The app falls back to `walkdir` silently if not elevated.

---

## Testing

```powershell
cargo test
```

40 integration and unit tests across `classifier`, `delta`, `entropy`, `treemap`, and `history` modules.

---

## Architecture

```
Scanner thread  ──mpsc batches──►  Arc<Mutex<DiskSnapshot>>  ──►  egui App
    │                                                                  │
    ├── MFT via DeviceIoControl (Admin)                       Treemap layout
    └── walkdir fallback (non-Admin / non-NTFS)               Delta sidebar
                                                              History panel
```

- The scanner runs in `std::thread::spawn` and streams `FileNode` batches over an `mpsc` channel; `rayon` parallelizes MFT entry processing.
- Treemap layout is recomputed only on snapshot change, not every frame.
- `history::save()` is called after each completed scan; keeps the latest 10 snapshots per drive.
- `delta::compute()` emits all changes including `Unchanged`; `apply_min_filter()` is the display-layer guard for the materiality threshold.

---

## License

Licensed under the [GNU General Public License v3.0](LICENSE). Any derivative work must also be distributed under GPL-3.0.

---

## Author

**Felipe Carvajal Brown**
