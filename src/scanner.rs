use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::types::FileNode;

pub enum ScanMsg {
    Batch(Vec<FileNode>),
    Done,
}

struct FrnEntry {
    name: String,
    parent_frn: u64,
    is_dir: bool,
    modified: SystemTime,
}

pub fn scan(drive: String, tx: Sender<ScanMsg>) {
    std::thread::spawn(move || {
        // try MFT first; falls back to walkdir if it fails (non-admin or non-NTFS)
        let result = scan_mft(&drive, &tx);
        if result.is_err() {
            scan_walkdir(&drive, &tx);
        }
        let _ = tx.send(ScanMsg::Done);
    });
}

fn scan_mft(drive: &str, tx: &Sender<ScanMsg>) -> Result<(), String> {
    let frn_map = build_frn_map(drive)?;
    let drive_root = drive.trim_end_matches('\\');

    // resolve paths and sample entropy in parallel across the thread pool
    let nodes: Vec<FileNode> = frn_map
        .par_iter()
        .filter(|(_, e)| !e.is_dir)
        .filter_map(|(&frn, entry)| {
            let path = resolve_path(frn, &frn_map, drive_root)?;
            let meta = std::fs::metadata(&path).ok()?;
            let mut node = FileNode {
                path,
                size_bytes: meta.len(),
                modified: entry.modified,
                entropy: None,
            };
            crate::entropy::sample_entropy(&mut node);
            Some(node)
        })
        .collect();

    for chunk in nodes.chunks(256) {
        tx.send(ScanMsg::Batch(chunk.to_vec()))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn build_frn_map(drive: &str) -> Result<HashMap<u64, FrnEntry>, String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Ioctl::{FSCTL_ENUM_USN_DATA, MFT_ENUM_DATA_V0, USN_RECORD_V2};
    use windows::Win32::System::IO::DeviceIoControl;

    let volume_path = format!("\\\\.\\{}", drive.trim_end_matches('\\'));
    let h: HANDLE = unsafe {
        CreateFileW(
            &HSTRING::from(volume_path.as_str()),
            0x0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
        .map_err(|e: windows::core::Error| e.to_string())?
    };

    let mut med = MFT_ENUM_DATA_V0 {
        StartFileReferenceNumber: 0,
        LowUsn: 0,
        HighUsn: i64::MAX,
    };

    let buf_size = 256 * 1024usize;
    let mut buf: Vec<u8> = vec![0u8; buf_size];
    let mut map: HashMap<u64, FrnEntry> = HashMap::new();
    let rec_hdr = std::mem::size_of::<USN_RECORD_V2>();

    loop {
        let mut bytes_returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                h,
                FSCTL_ENUM_USN_DATA,
                Some(&med as *const _ as *const _),
                std::mem::size_of::<MFT_ENUM_DATA_V0>() as u32,
                Some(buf.as_mut_ptr() as *mut _),
                buf_size as u32,
                Some(&mut bytes_returned),
                None,
            )
        };

        if ok.is_err() || bytes_returned < 8 {
            break;
        }

        med.StartFileReferenceNumber = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let valid = bytes_returned as usize;
        let mut offset = 8usize;

        while offset + rec_hdr <= valid {
            let rec = unsafe { &*(buf.as_ptr().add(offset) as *const USN_RECORD_V2) };
            let rec_len = rec.RecordLength as usize;
            if rec_len < rec_hdr {
                break;
            }

            let name_offset = rec.FileNameOffset as usize;
            let name_bytes = rec.FileNameLength as usize;
            let name_len = name_bytes / 2;
            let name_abs_end = offset.saturating_add(name_offset).saturating_add(name_bytes);

            if name_abs_end <= valid && name_len > 0 {
                let name_ptr = unsafe { buf.as_ptr().add(offset + name_offset) as *const u16 };
                let name = String::from_utf16_lossy(unsafe {
                    std::slice::from_raw_parts(name_ptr, name_len)
                });
                map.insert(rec.FileReferenceNumber as u64, FrnEntry {
                    name,
                    parent_frn: rec.ParentFileReferenceNumber as u64,
                    is_dir: rec.FileAttributes & 0x10 != 0,
                    modified: filetime_to_systemtime(rec.TimeStamp),
                });
            }

            offset = offset.saturating_add(rec_len);
        }
    }

    unsafe { let _ = CloseHandle(h); }

    if map.is_empty() {
        return Err("MFT enumeration returned no entries".into());
    }
    Ok(map)
}

fn resolve_path(frn: u64, map: &HashMap<u64, FrnEntry>, drive: &str) -> Option<PathBuf> {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = frn;
    for _ in 0..64 {
        let entry = map.get(&cur)?;
        if entry.parent_frn == cur {
            break; // root is self-parented on NTFS
        }
        parts.push(entry.name.clone());
        cur = entry.parent_frn;
    }
    if parts.is_empty() {
        return None;
    }
    parts.reverse();
    let mut path = PathBuf::from(drive);
    for part in parts {
        path.push(part);
    }
    Some(path)
}

fn scan_walkdir(drive: &str, tx: &Sender<ScanMsg>) {
    // par_bridge feeds the serial walkdir iterator into rayon's thread pool
    let nodes: Vec<FileNode> = WalkDir::new(drive)
        .follow_links(false)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .par_bridge()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            let modified = meta.modified().unwrap_or(UNIX_EPOCH.into());
            let mut node = FileNode {
                path: entry.path().to_path_buf(),
                size_bytes: meta.len(),
                modified,
                entropy: None,
            };
            crate::entropy::sample_entropy(&mut node);
            Some(node)
        })
        .collect();

    for chunk in nodes.chunks(256) {
        let _ = tx.send(ScanMsg::Batch(chunk.to_vec()));
    }
}

fn filetime_to_systemtime(ft: i64) -> SystemTime {
    // FILETIME is 100-nanosecond intervals since 1601-01-01
    const FILETIME_EPOCH_DIFF: u64 = 11_644_473_600;
    let secs = (ft as u64 / 10_000_000).saturating_sub(FILETIME_EPOCH_DIFF);
    UNIX_EPOCH + std::time::Duration::from_secs(secs)
}
