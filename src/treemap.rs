use crate::types::{ChurnClass, FolderStats};
use egui::{Color32, FontId, Painter, Pos2, Rect, Rounding, Stroke};

pub struct TreemapRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub folder_index: usize,
}

pub fn layout(folders: &[FolderStats], width: f32, height: f32) -> Vec<TreemapRect> {
    if folders.is_empty() || width <= 0.0 || height <= 0.0 {
        return vec![];
    }
    let total: u64 = folders.iter().map(|f| f.total_size).sum();
    if total == 0 {
        return vec![];
    }
    let area = width * height;
    let mut items: Vec<(usize, f32)> = folders
        .iter()
        .enumerate()
        .map(|(i, f)| (i, f.total_size as f32 / total as f32 * area))
        .collect();
    items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::with_capacity(folders.len());
    squarify(&items, 0.0, 0.0, width, height, &mut out);
    out
}

pub fn paint(painter: &Painter, rects: &[TreemapRect], folders: &[FolderStats], origin: Pos2) {
    for r in rects {
        let folder = &folders[r.folder_index];
        let rect = Rect::from_min_size(
            Pos2::new(origin.x + r.x, origin.y + r.y),
            egui::vec2(r.w, r.h),
        );
        painter.rect(
            rect,
            Rounding::same(2.0),
            churn_color(&folder.churn),
            Stroke::new(1.0, Color32::from_black_alpha(80)),
        );
        if r.w > 40.0 && r.h > 20.0 {
            let name = folder
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                name,
                FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
    }
}

fn churn_color(churn: &ChurnClass) -> Color32 {
    match churn {
        ChurnClass::Cold => Color32::from_rgb(60, 100, 160),
        ChurnClass::Hot => Color32::from_rgb(200, 80, 40),
        ChurnClass::Volatile => Color32::from_rgb(210, 170, 30),
    }
}

fn squarify(items: &[(usize, f32)], x: f32, y: f32, w: f32, h: f32, out: &mut Vec<TreemapRect>) {
    if items.is_empty() || w < 1.0 || h < 1.0 {
        return;
    }
    let short = w.min(h);
    let mut row_end = 0;
    loop {
        if row_end == items.len() {
            place_row(&items[..row_end], x, y, w, h, out);
            return;
        }
        let next = row_end + 1;
        let cur = if row_end == 0 { f32::MAX } else { worst_ratio(&items[..row_end], short) };
        if row_end == 0 || worst_ratio(&items[..next], short) <= cur {
            row_end = next;
        } else {
            place_row(&items[..row_end], x, y, w, h, out);
            let (nx, ny, nw, nh) = remaining_rect(&items[..row_end], x, y, w, h);
            squarify(&items[row_end..], nx, ny, nw, nh, out);
            return;
        }
    }
}

fn worst_ratio(row: &[(usize, f32)], short: f32) -> f32 {
    let sum: f32 = row.iter().map(|(_, s)| s).sum();
    if sum == 0.0 || short == 0.0 {
        return f32::MAX;
    }
    let max_s = row.iter().map(|(_, s)| *s).fold(f32::NEG_INFINITY, f32::max);
    let min_s = row.iter().map(|(_, s)| *s).fold(f32::INFINITY, f32::min);
    let s2 = sum * sum;
    let sh2 = short * short;
    f32::max(sh2 * max_s / s2, s2 / (sh2 * min_s))
}

fn place_row(row: &[(usize, f32)], x: f32, y: f32, w: f32, h: f32, out: &mut Vec<TreemapRect>) {
    let sum: f32 = row.iter().map(|(_, s)| s).sum();
    if sum == 0.0 {
        return;
    }
    if w >= h {
        let strip_w = sum / h;
        let mut cy = y;
        for (idx, s) in row {
            let ih = s / strip_w;
            out.push(TreemapRect { x, y: cy, w: strip_w, h: ih, folder_index: *idx });
            cy += ih;
        }
    } else {
        let strip_h = sum / w;
        let mut cx = x;
        for (idx, s) in row {
            let iw = s / strip_h;
            out.push(TreemapRect { x: cx, y, w: iw, h: strip_h, folder_index: *idx });
            cx += iw;
        }
    }
}

fn remaining_rect(row: &[(usize, f32)], x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let sum: f32 = row.iter().map(|(_, s)| s).sum();
    if w >= h {
        let strip_w = sum / h;
        (x + strip_w, y, w - strip_w, h)
    } else {
        let strip_h = sum / w;
        (x, y + strip_h, w, h - strip_h)
    }
}
