# DiskChurn — Product Requirements Document

## Overview

DiskChurn is a native Windows disk space analyzer built in Rust. It does what WinDirStat and WizTree do — show you what is eating your disk — but adds two features none of them have: **storage growth prediction and file churn classification computed from a single scan**, and a **Reclaimable Space Score** based on Shannon entropy sampling that tells you which folders are worth compressing and which are already at maximum density.

Author: Felipe Carvajal Brown Software

---

## Problem

Existing tools (WinDirStat, WizTree, TreeSize) answer "what is large right now." They do not answer "what is growing fast," "when will my disk fill up," or "which of these folders would actually shrink if I compressed them." TreeSize has trend analysis but requires saving multiple snapshots over days or weeks. No tool classifies folder behavior from a single scan, and no tool tells you whether the space a folder occupies is genuinely compressible or already as dense as it can get.

---

## Core Novel Feature: Churn Classification + Growth Projection

Every NTFS file stores three timestamps in `$STANDARD_INFORMATION`: created, last modified, last accessed. By reading these from the MFT directly (same technique WizTree uses for speed), DiskChurn can classify every folder into one of three behavioral categories in a single pass:

- **Cold** — large, untouched. Files mostly created >90 days ago, rarely modified. Examples: movie libraries, old game installs, photo archives. Safe to move to external storage.
- **Hot** — actively growing. High ratio of recently modified/created files, total size trending up. Examples: Steam, Downloads, AppData. Shows projected days until disk full via linear regression on file modification timestamps.
- **Volatile** — high file count churn, small average file size, short file lifetimes. Examples: temp folders, browser caches, build artifacts, log directories. Primary deletion candidates.

The growth projection algorithm: within each Hot folder, sort files by modification date, fit a linear regression on cumulative size vs. time, extrapolate to disk capacity. Displayed as "~47 days until full at current rate."

This is not approximate — the data is already in the MFT. It is just never surfaced.

---

## Second Novel Feature: Reclaimable Space Score via Shannon Entropy Sampling

Every file has a compressibility ceiling determined by its entropy. Files with low Shannon entropy (text, logs, uncompressed BMP/WAV, raw VM images, uncompressed game assets) compress well. Files with high entropy (ZIPs, JPEGs, MP4s, encrypted containers) cannot be compressed further — attempting NTFS compression on them wastes CPU and gains nothing.

No existing disk analyzer surfaces this. Extension-based guessing is unreliable: a PDF can be a vector document that compresses 60%, or a container of already-compressed JPEGs that compresses 0%. A WAV can be white noise (incompressible) or a mono track padded to stereo (highly compressible).

DiskChurn samples the first 64 KB of each file during the scan pass, computes its Shannon entropy (0.0–8.0 bits/byte scale), and rolls it up per folder into a **Reclaimable Space Score**: an estimated percentage and GB that could be recovered by enabling NTFS compression or archiving.

Entropy thresholds:
- **< 6.0 bits/byte** — compressible, flag as reclaimable
- **6.0–7.2** — mixed, marginal gain
- **> 7.2 bits/byte** — already dense, compression pointless

The 64 KB sample cap keeps the entropy pass fast even on large drives. The score is additive with churn class, giving the user a combined action recommendation:

| Folder | Churn | Entropy Score | Recommended Action |
|---|---|---|---|
| `C:/RawPhotos` | Cold | Low | Enable NTFS compression — est. ~38% saving |
| `C:/Videos` | Cold | High | Already dense, archive to external instead |
| `C:/Logs` | Volatile | Low | Delete — compression won't help long-term |
| `C:/Steam/GameX` | Cold | Mixed | Per-subfolder breakdown available |

The entropy sampler runs on the same background thread as the MFT walk, reading file content in a second pass after metadata is collected. It is skipped automatically for files already marked as NTFS-compressed in the MFT attributes.

---

## Tech Stack

| Layer | Choice | Reason |
|---|---|---|
| Language | Rust | Performance, memory safety, single binary |
| GUI | egui + eframe | Immediate mode, no webview, ships as .exe |
| Charts | egui_plot | Native egui, bar + treemap |
| Disk scan | windows-rs (DeviceIoControl + FSCTL_ENUM_USN_DATA) | Direct MFT access, same speed as WizTree |
| Fallback scan | walkdir | Non-admin mode or non-NTFS drives |
| Drive enumeration | GetLogicalDriveStringsW via windows-rs | Lists C:, D:, etc. for dropdown |
| Entropy sampling | std::fs::File + manual Shannon calculation | No extra crate needed, 64 KB sample per file |

---

## Features

### Must Have (v0.1)

- Drive selector dropdown (C:, D:, any detected NTFS volume)
- Full disk scan via MFT (admin) or walkdir fallback (non-admin)
- Treemap visualization — folder size as area, colored by churn class (cold=blue, hot=orange, volatile=red)
- Folder list panel — sorted by size, shows churn class badge and growth projection for Hot folders
- Churn classification engine — single-pass, computed from MFT timestamps
- Growth projection — linear regression, shown as "N days until full" on Hot folders
- Shannon entropy sampler — 64 KB sample per file, second pass after MFT walk
- Reclaimable Space Score — per-folder estimated GB reclaimable via compression, shown in folder list
- Scan runs on a background thread, UI stays responsive with a progress bar
- Filter sidebar: show only Cold / Hot / Volatile; filter by min size (e.g. >1 GB only); filter by entropy class (Compressible / Dense / Mixed)

### Nice to Have (v0.2)

- Click treemap cell to drill into subfolder
- Export scan results as CSV
- Rescan button (diff against previous scan, highlight changed folders)
- Dark/light theme toggle

### Out of Scope

- File deletion (read-only tool, same philosophy as WinDirStat)
- Network drives
- Non-Windows platforms

---

## File Structure

```
diskchurn/
├── Cargo.toml
├── build.rs
├── src/
│   ├── main.rs          # eframe::run_native bootstrap, window config
│   ├── app.rs           # egui App impl — layout, panels, drive dropdown, state machine
│   ├── scanner.rs       # MFT walk via DeviceIoControl, walkdir fallback, sends FileNode via channel
│   ├── classifier.rs    # ChurnClass assignment + linear regression for growth projection
│   ├── entropy.rs       # 64 KB file sampler, Shannon entropy calc, EntropyClass assignment
│   ├── treemap.rs       # Squarify layout algorithm, renders via egui painter
│   └── types.rs         # FileNode, FolderStats, ChurnClass, EntropyClass, DiskSnapshot
```

---

## Key Types

```rust
pub enum ChurnClass { Cold, Hot, Volatile }

pub enum EntropyClass { Compressible, Mixed, Dense }

pub struct FileNode {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub entropy: Option<f32>, // None until entropy pass runs; 0.0–8.0 scale
    pub ntfs_compressed: bool, // skip entropy sample if already compressed
}

pub struct FolderStats {
    pub path: PathBuf,
    pub total_size: u64,
    pub file_count: u64,
    pub churn: ChurnClass,
    pub entropy_class: EntropyClass,
    pub days_until_full: Option<f32>,     // Some only for Hot
    pub reclaimable_bytes: Option<u64>,   // Some only for Compressible/Mixed
    pub children: Vec<FolderStats>,
}
```

---

## Architecture Notes

- Scanner runs in `std::thread::spawn`, sends `FileNode` batches over `std::sync::mpsc`
- App holds `Arc<Mutex<DiskSnapshot>>` updated as batches arrive
- Treemap layout is recomputed only when snapshot changes (not every frame)
- MFT scan requires the process to run as Administrator; fallback to walkdir is automatic and silent
- `build.rs` sets `/SUBSYSTEM:WINDOWS` so no console window spawns

---

## UI Layout

```
[Drive dropdown: C: v]  [Scan]  [progress bar................]

[Filter: All | Cold | Hot | Volatile]  [Entropy: All | Compressible | Dense]  [Min size: 0 MB --o-- 10 GB]

+-----------------------------+----------------------------------------------------------+
|  Treemap                    |  Folder list                                             |
|                             |  /Users/Felipe/Downloads  HOT   COMPRESSIBLE             |
|   [colored rectangles]      |  42 GB  ~31 days until full  ~14 GB reclaimable          |
|                             |                                                          |
|                             |  C:/Windows/Temp  VOLATILE  COMPRESSIBLE                 |
|                             |  8 GB  14,200 files  ~3 GB reclaimable                   |
|                             |                                                          |
|                             |  C:/Games/Cyberpunk  COLD  DENSE                         |
|                             |  70 GB  last touched 4mo ago  already compressed         |
+-----------------------------+----------------------------------------------------------+
```

---

## Crate Dependencies

```toml
[dependencies]
eframe = "0.27"
egui = "0.27"
egui_plot = "0.27"
walkdir = "2"

[dependencies.windows]
version = "0.56"
features = [
    "Win32_System_Ioctl",
    "Win32_System_SystemInformation",
    "Win32_Storage_FileSystem",
    "Win32_Foundation",
]
```

---

## Coding Rules

- One-line comments only, informal tone
- No multi-line or block comments
- No emojis in code, docs, or commit messages
- Commit messages: single short lines
- Branding: "Felipe Carvajal Brown Software" in Cargo.toml authors field
- Root-cause fixes only — no test workarounds
