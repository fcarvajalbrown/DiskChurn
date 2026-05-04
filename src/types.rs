use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
pub enum ChurnClass {
    Cold,
    Hot,
    Volatile,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntropyClass {
    Compressible,
    Mixed,
    Dense,
}

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: SystemTime,
    pub entropy: Option<f32>, // 0.0–8.0 scale, None until entropy pass
}

#[derive(Debug, Clone)]
pub struct FolderStats {
    pub path: PathBuf,
    pub total_size: u64,
    pub file_count: u64,
    pub churn: ChurnClass,
    pub entropy_class: EntropyClass,
    pub days_until_full: Option<f32>,
}

// full point-in-time picture of a scan
#[derive(Debug, Default, Clone)]
pub struct DiskSnapshot {
    pub drive: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub files: Vec<FileNode>,
    pub folders: Vec<FolderStats>,
    pub scan_complete: bool,
    pub files_scanned: u64,
}
