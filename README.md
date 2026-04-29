# DiskChurn

Windows disk space analyzer built in Rust. Shows what is eating your disk, classifies folders by behavior, and tells you which ones are worth compressing — all from a single scan.

## What it does

- **Churn classification** — reads NTFS MFT timestamps to label every folder as Cold (untouched), Hot (actively growing), or Volatile (high file turnover). No multi-day snapshot required.
- **Growth projection** — for Hot folders, fits a linear regression on modification timestamps and projects days until disk full.
- **Reclaimable Space Score** — samples the first 64 KB of each file, computes Shannon entropy, and estimates how much space NTFS compression would actually recover. Skips files that are already compressed or high-entropy (ZIPs, JPEGs, MP4s).
- **Treemap view** — folders rendered as area-proportional rectangles, colored by churn class.

## Why

WinDirStat and WizTree answer "what is large right now." They do not answer "what is growing," "when will this disk fill up," or "is this folder actually compressible." DiskChurn answers all three from one scan pass.

## Tech stack

| Layer | Choice |
|---|---|
| Language | Rust |
| GUI | egui + eframe 0.27 |
| Disk scan | windows-rs (MFT via DeviceIoControl) |
| Fallback scan | walkdir (non-admin or non-NTFS) |
| Charts | egui_plot |

## Building

Requires Rust stable and Windows. Must run as Administrator for MFT access; falls back to walkdir automatically if not.

```
cargo build --release
```

Binary outputs to `target/release/diskchurn.exe`. No installer, no dependencies.

## Status

v0.1 in progress. See [docs/prd.md](docs/prd.md) for full feature spec.

## Author

Felipe Carvajal Brown
