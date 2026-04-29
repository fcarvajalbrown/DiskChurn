// stub — full implementation coming next
use crate::types::FolderStats;

pub struct TreemapRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub folder_index: usize,
}

pub fn layout(_folders: &[FolderStats], _width: f32, _height: f32) -> Vec<TreemapRect> {
    vec![]
}
