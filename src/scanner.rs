use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};

use walkdir::WalkDir;

use crate::types::FileNode;

pub enum ScanMsg {
    Batch(Vec<FileNode>),
    Done,
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
    use windows::core::HSTRING;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows::Win32::System::Ioctl::{
        FSCTL_ENUM_USN_DATA, MFT_ENUM_DATA_V0, USN_RECORD_V2,
    };
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

    let buf_size = 64 * 1024usize;
    let mut buf: Vec<u8> = vec![0u8; buf_size];
    let mut batch: Vec<FileNode> = Vec::with_capacity(256);

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

        if ok.is_err() {
            break;
        }

        // first 8 bytes are the next StartFileReferenceNumber
        if bytes_returned < 8 {
            break;
        }
        med.StartFileReferenceNumber = u64::from_le_bytes(buf[0..8].try_into().unwrap());

        let mut offset = 8usize;
        while offset + std::mem::size_of::<USN_RECORD_V2>() <= bytes_returned as usize {
            let rec = unsafe { &*(buf.as_ptr().add(offset) as *const USN_RECORD_V2) };
            if rec.RecordLength == 0 {
                break;
            }

            // only emit file records, skip directories
            if rec.FileAttributes & 0x10 == 0 {
                let name_offset = rec.FileNameOffset as usize;
                let name_len = rec.FileNameLength as usize / 2;
                let name_ptr = unsafe { buf.as_ptr().add(offset + name_offset) as *const u16 };
                let name_slice = unsafe { std::slice::from_raw_parts(name_ptr, name_len) };
                let name = String::from_utf16_lossy(name_slice);

                // partial path — full resolution requires parent FRN map (v0.2)
                let path = PathBuf::from(format!("{}\\...\\{}", drive, name));
                let modified = filetime_to_systemtime(rec.TimeStamp);

                batch.push(FileNode {
                    path,
                    size_bytes: 0, // USN doesn't carry size; entropy pass fills this
                    modified,
                    entropy: None,
                });

                if batch.len() >= 256 {
                    let _ = tx.send(ScanMsg::Batch(std::mem::take(&mut batch)));
                    batch = Vec::with_capacity(256);
                }
            }

            offset += rec.RecordLength as usize;
        }
    }

    if !batch.is_empty() {
        let _ = tx.send(ScanMsg::Batch(batch));
    }

    Ok(())
}

fn scan_walkdir(drive: &str, tx: &Sender<ScanMsg>) {
    let mut batch: Vec<FileNode> = Vec::with_capacity(256);

    for entry in WalkDir::new(drive).follow_links(false).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path().to_path_buf();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let modified = meta.modified().unwrap_or(UNIX_EPOCH.into());

        batch.push(FileNode {
            path,
            size_bytes: meta.len(),
            modified,
            entropy: None,
        });

        if batch.len() >= 256 {
            let _ = tx.send(ScanMsg::Batch(std::mem::take(&mut batch)));
            batch = Vec::with_capacity(256);
        }
    }

    if !batch.is_empty() {
        let _ = tx.send(ScanMsg::Batch(batch));
    }
}

fn filetime_to_systemtime(ft: i64) -> SystemTime {
    // FILETIME is 100-nanosecond intervals since 1601-01-01
    const FILETIME_EPOCH_DIFF: u64 = 11_644_473_600;
    let secs = (ft as u64 / 10_000_000).saturating_sub(FILETIME_EPOCH_DIFF);
    UNIX_EPOCH + std::time::Duration::from_secs(secs)
}
