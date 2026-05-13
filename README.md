# DiskChurn

![Platform](https://img.shields.io/badge/platform-Windows-0078D4?logo=windows&logoColor=white)
![Language](https://img.shields.io/badge/language-Rust-orange?logo=rust&logoColor=white)
![GUI](https://img.shields.io/badge/GUI-egui%200.27-blue)
![Tests](https://img.shields.io/badge/tests-40%20passing-brightgreen)
![Version](https://img.shields.io/badge/version-0.1-informational)

Windows disk space analyzer built in Rust. Shows what is eating your disk, classifies folders by behavior, compares snapshots over time, and tells you which ones are worth compressing — all from a single scan.

## What it does

- **Churn classification** — reads NTFS MFT timestamps to label every folder as Cold (untouched), Hot (actively growing), or Volatile (high file turnover). No multi-day snapshot required.
- **Growth projection** — for Hot folders, fits a linear regression on modification timestamps and projects days until disk full.
- **Entropy analysis** — samples the first 64 KB of each file, computes Shannon entropy, and estimates how much space compression would actually recover. Skips files that are already dense (ZIPs, JPEGs, MP4s).
- **Treemap view** — folders rendered as area-proportional rectangles, colored by churn class.
- **Scan history** — saves up to 10 snapshots per drive to `%APPDATA%\DiskChurn\snapshots\` automatically after each scan.
- **Delta view** — compare any two snapshots to see which folders grew, shrank, appeared, or disappeared. User-controlled materiality threshold via log-scale slider (0 to 10 GB).

## Why

WinDirStat and WizTree answer "what is large right now." They do not answer "what is growing," "when will this disk fill up," or "is this folder actually compressible." DiskChurn answers all three from one scan pass, and lets you track changes across time.

## Tech stack

| Layer | Choice |
|---|---|
| Language | Rust (edition 2021) |
| GUI | egui + eframe 0.27 |
| Disk scan | windows-rs (MFT via `DeviceIoControl`) |
| Fallback scan | walkdir (non-admin or non-NTFS) |
| Serialization | bincode + serde |
| Parallelism | rayon |

## Building

Requires Rust stable and Windows with the MSVC C++ workload. Must run as Administrator for MFT access; falls back to walkdir automatically if not.

```
cargo build --release
```

Binary outputs to `target/release/diskchurn.exe`. No installer, no runtime dependencies.

## Testing

```
cargo test
```

40 tests across classifier, delta, entropy, treemap, and history modules.

## Author

Felipe Carvajal Brown
