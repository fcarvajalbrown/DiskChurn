use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Receiver;

use eframe::egui;
use egui::{Color32, ComboBox, RichText, ScrollArea, Sense};

use crate::{
    classifier, delta, history,
    delta::{DeltaKind, FolderDelta},
    scanner::{self, ScanMsg},
    treemap::{self, TreemapRect},
    types::{ChurnClass, DiskSnapshot, EntropyClass, FolderStats},
};

enum ScanState {
    Idle,
    Scanning,
    Done,
}

pub struct DiskChurnApp {
    snapshot: Arc<Mutex<DiskSnapshot>>,
    rx: Option<Receiver<ScanMsg>>,
    state: ScanState,
    rects: Vec<TreemapRect>,
    display_folders: Vec<FolderStats>,
    dirty: bool,
    drives: Vec<String>,
    selected_drive: String,
    filter_churn: Option<ChurnClass>,
    selected_folder: Option<PathBuf>,
    treemap_size: egui::Vec2,
    history: Vec<(u64, u64, PathBuf)>,
    compare_idx: Option<usize>,
    delta: Vec<FolderDelta>,
    display_delta: Vec<FolderDelta>,
    delta_rects: Vec<TreemapRect>,
    delta_size: egui::Vec2,
    delta_min_bytes: u64,
    delta_show: [bool; 4], // New, Grown, Shrunk, Deleted
}

impl Default for DiskChurnApp {
    fn default() -> Self {
        let drives = detect_drives();
        let selected = drives.first().cloned().unwrap_or_else(|| "C:\\".into());
        Self {
            snapshot: Arc::new(Mutex::new(DiskSnapshot::default())),
            rx: None,
            state: ScanState::Idle,
            rects: vec![],
            display_folders: vec![],
            dirty: false,
            drives,
            selected_drive: selected,
            filter_churn: None,
            selected_folder: None,
            treemap_size: egui::Vec2::ZERO,
            history: vec![],
            compare_idx: None,
            delta: vec![],
            display_delta: vec![],
            delta_rects: vec![],
            delta_size: egui::Vec2::ZERO,
            delta_min_bytes: 0,
            delta_show: [true; 4],
        }
    }
}

impl eframe::App for DiskChurnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_rx(ctx);
        self.draw_toolbar(ctx);
        self.draw_detail(ctx);
        self.draw_sidebar(ctx);
        self.draw_treemap(ctx);
    }
}

impl DiskChurnApp {
    fn drain_rx(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        let mut done = false;
        {
            let mut snap = self.snapshot.lock().unwrap();
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ScanMsg::Batch(files) => {
                        snap.files.extend(files);
                        snap.files_scanned = snap.files.len() as u64;
                    }
                    ScanMsg::Done => {
                        snap.scan_complete = true;
                        done = true;
                    }
                }
            }
            if done {
                let (total, free) = disk_space(&snap.drive);
                snap.total_bytes = total;
                snap.free_bytes = free;
                snap.folders = classifier::classify(&snap.files, total, free);
            }
        }
        if done {
            self.state = ScanState::Done;
            self.rx = None;
            self.dirty = true;
            {
                let snap = self.snapshot.lock().unwrap();
                history::save(&snap);
            }
            self.history = history::list(&self.selected_drive);
        }
        ctx.request_repaint();
    }

    fn start_scan(&mut self) {
        {
            let mut snap = self.snapshot.lock().unwrap();
            *snap = DiskSnapshot { drive: self.selected_drive.clone(), ..Default::default() };
        }
        self.rects.clear();
        self.display_folders.clear();
        self.selected_folder = None;
        self.dirty = false;
        self.state = ScanState::Scanning;
        self.compare_idx = None;
        self.delta.clear();
        self.display_delta.clear();
        self.delta_rects.clear();
        self.history = history::list(&self.selected_drive);
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        scanner::scan(self.selected_drive.clone(), tx);
    }

    fn draw_detail(&mut self, ctx: &egui::Context) {
        let Some(ref selected) = self.selected_folder else { return };

        let delta_entry = self.delta.iter().find(|d| d.path == *selected).cloned();

        let (folder_stats, top_files) = {
            let snap = self.snapshot.lock().unwrap();
            let stats = snap.folders.iter().find(|f| f.path == *selected).cloned();
            let mut files: Vec<(String, u64)> = snap
                .files
                .iter()
                .filter(|f| f.path.parent() == Some(selected.as_path()))
                .map(|f| {
                    let name = f
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    (name, f.size_bytes)
                })
                .collect();
            files.sort_by(|a, b| b.1.cmp(&a.1));
            files.truncate(10);
            (stats, files)
        };

        let Some(stats) = folder_stats else { return };

        egui::TopBottomPanel::bottom("detail").min_height(160.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong(stats.path.to_string_lossy().as_ref());
                ui.separator();
                ui.label(format!(
                    "{:.0} MB total  |  {} files  |  {:?}  |  {:?}",
                    stats.total_size as f64 / 1e6,
                    stats.file_count,
                    stats.churn,
                    stats.entropy_class,
                ));
                if let Some(ref d) = delta_entry {
                    ui.separator();
                    ui.colored_label(
                        delta_sidebar_color(&d.kind),
                        format!("{:?}  {}", d.kind, treemap::fmt_delta(d.delta)),
                    );
                }
                if let Some(days) = stats.days_until_full {
                    ui.separator();
                    ui.colored_label(
                        Color32::from_rgb(220, 100, 60),
                        format!("disk full in ~{:.0} days", days),
                    );
                }
            });
            ui.separator();
            ScrollArea::vertical().id_source("detail_scroll").show(ui, |ui| {
                for (name, size) in &top_files {
                    ui.horizontal(|ui| {
                        ui.monospace(fmt_size(*size));
                        ui.label(name);
                    });
                }
                if top_files.is_empty() {
                    ui.label("no files found");
                }
            });
        });
    }

    fn draw_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let scanning = matches!(self.state, ScanState::Scanning);

                ui.add_enabled_ui(!scanning, |ui| {
                    let cur = self.selected_drive.clone();
                    ComboBox::from_id_source("drive_sel")
                        .selected_text(&cur)
                        .show_ui(ui, |ui| {
                            for d in &self.drives {
                                ui.selectable_value(&mut self.selected_drive, d.clone(), d.as_str());
                            }
                        });
                    if ui.button("Scan").clicked() {
                        self.start_scan();
                    }
                });

                ui.separator();
                {
                    let snap = self.snapshot.lock().unwrap();
                    match self.state {
                        ScanState::Idle => { ui.label("idle"); }
                        ScanState::Scanning => {
                            ui.spinner();
                            ui.label(format!("scanning… {} files", snap.files_scanned));
                        }
                        ScanState::Done => {
                            ui.label(format!(
                                "{} files  |  {:.1} GB total  |  {:.1} GB free",
                                snap.files_scanned,
                                snap.total_bytes as f64 / 1e9,
                                snap.free_bytes as f64 / 1e9,
                            ));
                        }
                    }
                }

                if !matches!(self.state, ScanState::Idle) {
                    ui.separator();
                    ui.label("filter:");
                    for (label, class) in [
                        ("cold", ChurnClass::Cold),
                        ("hot", ChurnClass::Hot),
                        ("volatile", ChurnClass::Volatile),
                    ] {
                        let active = self.filter_churn.as_ref() == Some(&class) && self.compare_idx.is_none();
                        if ui.add_enabled(self.compare_idx.is_none(), egui::SelectableLabel::new(active, label)).clicked() {
                            self.filter_churn = if active { None } else { Some(class) };
                            self.selected_folder = None;
                            self.dirty = true;
                        }
                    }
                }

                if matches!(self.state, ScanState::Done) && !self.history.is_empty() {
                    ui.separator();
                    ui.label("compare:");
                    let cur_label = self.compare_idx
                        .and_then(|i| self.history.get(i))
                        .map(|(ts, fc, _)| format!("{} ({} files)", fmt_age(*ts), fc))
                        .unwrap_or_else(|| "none".into());
                    let mut load_idx: Option<usize> = None;
                    let mut clear = false;
                    ComboBox::from_id_source("compare_sel")
                        .selected_text(cur_label)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(self.compare_idx.is_none(), "none").clicked() {
                                clear = true;
                            }
                            for (i, (ts, fc, _)) in self.history.iter().enumerate() {
                                let sel = self.compare_idx == Some(i);
                                let lbl = format!("{} ({} files)", fmt_age(*ts), fc);
                                if ui.selectable_label(sel, lbl).clicked() && !sel {
                                    load_idx = Some(i);
                                }
                            }
                        });
                    if clear && self.compare_idx.is_some() {
                        self.compare_idx = None;
                        self.delta.clear();
                        self.display_delta.clear();
                        self.selected_folder = None;
                        self.dirty = true;
                    }
                    if let Some(idx) = load_idx {
                        let path = self.history[idx].2.clone();
                        if let Some(snap) = history::load(&path) {
                            let current = self.snapshot.lock().unwrap();
                            self.delta = delta::compute(&snap, &current.folders);
                            drop(current);
                            self.compare_idx = Some(idx);
                            self.selected_folder = None;
                            self.dirty = true;
                        }
                    }
                }

                if self.compare_idx.is_some() {
                    ui.separator();
                    let prev_min = self.delta_min_bytes;
                    ui.label("min:");
                    ui.add(
                        egui::Slider::new(&mut self.delta_min_bytes, 0u64..=10_737_418_240u64)
                            .logarithmic(true)
                            .custom_formatter(|v, _| {
                                if v < 1.0 { "0 (all)".into() } else { fmt_size(v as u64) }
                            })
                            .text(""),
                    );
                    if self.delta_min_bytes != prev_min {
                        self.dirty = true;
                    }

                    ui.separator();
                    let prev_show = self.delta_show;
                    for (i, label) in ["new", "grown", "shrunk", "deleted"].iter().enumerate() {
                        if ui.selectable_label(self.delta_show[i], *label).clicked() {
                            self.delta_show[i] = !self.delta_show[i];
                        }
                    }
                    if self.delta_show != prev_show {
                        self.dirty = true;
                    }
                }
            });
        });
    }

    fn draw_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar").min_width(220.0).show(ctx, |ui| {
            if self.compare_idx.is_some() {
                ui.heading("Changes");
                ui.separator();
                let mut sorted = delta::apply_min_filter(&self.delta, self.delta_min_bytes, self.delta_show);
                sorted.sort_by(|a, b| b.delta.unsigned_abs().cmp(&a.delta.unsigned_abs()));
                const SIDEBAR_CAP: usize = 500;
                let overflow = sorted.len().saturating_sub(SIDEBAR_CAP);
                ScrollArea::vertical().show(ui, |ui| {
                    for d in sorted.iter().take(SIDEBAR_CAP) {
                        let selected = self.selected_folder.as_deref() == Some(d.path.as_path());
                        let name = d.path.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| d.path.to_string_lossy().into_owned());
                        let kind_str = match d.kind {
                            DeltaKind::New => "new",
                            DeltaKind::Deleted => "deleted",
                            DeltaKind::Grown => "grown",
                            DeltaKind::Shrunk => "shrunk",
                            DeltaKind::Unchanged => "unchanged",
                        };
                        let label = format!(
                            "{}\n{}  |  {}",
                            name,
                            treemap::fmt_delta(d.delta),
                            kind_str,
                        );
                        let color = delta_sidebar_color(&d.kind);
                        if ui.selectable_label(selected, RichText::new(label).color(color)).clicked() {
                            self.selected_folder = if selected { None } else { Some(d.path.clone()) };
                        }
                    }
                    if overflow > 0 {
                        ui.separator();
                        ui.label(format!("{} more — raise min change to narrow", overflow));
                    }
                    if sorted.is_empty() {
                        ui.label("no changes above threshold");
                    }
                });
                return;
            }

            ui.heading("Folders");
            ui.separator();
            let snap = self.snapshot.lock().unwrap();
            let mut rows: Vec<(usize, &FolderStats)> = snap
                .folders
                .iter()
                .enumerate()
                .filter(|(_, f)| self.filter_churn.as_ref().map_or(true, |c| &f.churn == c))
                .collect();
            rows.sort_by(|a, b| b.1.total_size.cmp(&a.1.total_size));
            ScrollArea::vertical().show(ui, |ui| {
                for (_, folder) in &rows {
                    let selected = self.selected_folder.as_deref() == Some(folder.path.as_path());
                    let name = folder
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| folder.path.to_string_lossy().into_owned());
                    let entropy_label = match folder.entropy_class {
                        EntropyClass::Compressible => "compressible",
                        EntropyClass::Mixed => "mixed",
                        EntropyClass::Dense => "dense",
                    };
                    let label = format!(
                        "{}\n{:.0} MB  |  {} files  |  {}",
                        name,
                        folder.total_size as f64 / 1e6,
                        folder.file_count,
                        entropy_label,
                    );
                    let color = churn_color(folder.churn.clone());
                    if ui
                        .selectable_label(selected, RichText::new(label).color(color))
                        .clicked()
                    {
                        self.selected_folder = if selected { None } else { Some(folder.path.clone()) };
                    }
                }
            });
        });
    }

    fn draw_treemap(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let size = ui.available_size();

            if self.compare_idx.is_some() {
                if self.dirty || (size - self.delta_size).length() > 1.0 {
                    self.display_delta = delta::apply_min_filter(&self.delta, self.delta_min_bytes, self.delta_show)
                        .into_iter()
                        .cloned()
                        .collect();
                    self.display_delta.sort_by(|a, b| b.delta.unsigned_abs().cmp(&a.delta.unsigned_abs()));

                    // drop sub-pixel entries before squarify to prevent hairline slivers
                    let treemap_area = size.x * size.y;
                    let total_delta: u64 = self.display_delta.iter().map(|d| d.delta.unsigned_abs()).sum();
                    let sizes: Vec<(usize, u64)> = self.display_delta
                        .iter()
                        .enumerate()
                        .filter(|(_, d)| {
                            total_delta == 0 || d.delta.unsigned_abs() as f32 / total_delta as f32 * treemap_area >= 1.0
                        })
                        .map(|(i, d)| (i, d.delta.unsigned_abs()))
                        .collect();
                    self.delta_rects = treemap::layout_from_sizes(&sizes, size.x, size.y);
                    self.delta_size = size;
                    self.dirty = false;
                }
                if self.display_delta.is_empty() {
                    ui.label("no changes detected between snapshots");
                    return;
                }
                let origin = ui.min_rect().min;
                for r in &self.delta_rects {
                    let d = &self.display_delta[r.folder_index];
                    let abs_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(origin.x + r.x, origin.y + r.y),
                        egui::vec2(r.w, r.h),
                    );
                    let response = ui.interact(abs_rect, ui.id().with(("d", r.folder_index)), Sense::click());
                    if response.clicked() {
                        let path = d.path.clone();
                        let already = self.selected_folder.as_deref() == Some(path.as_path());
                        self.selected_folder = if already { None } else { Some(path) };
                    }
                    response.on_hover_ui(|ui| {
                        ui.label(d.path.to_string_lossy().as_ref());
                        ui.label(format!(
                            "{}  ({:.1} MB -> {:.1} MB)",
                            treemap::fmt_delta(d.delta),
                            d.old_size as f64 / 1e6,
                            d.new_size as f64 / 1e6,
                        ));
                    });
                }
                let painter = ui.painter().clone();
                treemap::paint_delta(&painter, &self.delta_rects, &self.display_delta, origin, self.selected_folder.as_deref());
                return;
            }

            if self.dirty || (size - self.treemap_size).length() > 1.0 {
                let snap = self.snapshot.lock().unwrap();
                self.display_folders = snap
                    .folders
                    .iter()
                    .filter(|f| self.filter_churn.as_ref().map_or(true, |c| &f.churn == c))
                    .cloned()
                    .collect();
                drop(snap);
                self.rects = treemap::layout(&self.display_folders, size.x, size.y);
                self.treemap_size = size;
                self.dirty = false;
            }
            if self.display_folders.is_empty() {
                ui.label(match self.state {
                    ScanState::Idle => "select a drive and click Scan",
                    ScanState::Scanning => "scanning…",
                    ScanState::Done => "no folders to display",
                });
                return;
            }
            let origin = ui.min_rect().min;
            for r in &self.rects {
                let folder = &self.display_folders[r.folder_index];
                let abs_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(origin.x + r.x, origin.y + r.y),
                    egui::vec2(r.w, r.h),
                );
                let response = ui.interact(abs_rect, ui.id().with(r.folder_index), Sense::click());
                if response.clicked() {
                    let path = folder.path.clone();
                    let already = self.selected_folder.as_deref() == Some(path.as_path());
                    self.selected_folder = if already { None } else { Some(path) };
                }
                response.on_hover_ui(|ui| {
                    ui.label(folder.path.to_string_lossy().as_ref());
                    ui.label(format!(
                        "{:.1} MB  |  {} files  |  {:?}",
                        folder.total_size as f64 / 1e6,
                        folder.file_count,
                        folder.churn,
                    ));
                });
            }
            let painter = ui.painter().clone();
            treemap::paint(&painter, &self.rects, &self.display_folders, origin, self.selected_folder.as_deref());
        });
    }
}

fn detect_drives() -> Vec<String> {
    (b'A'..=b'Z')
        .map(|c| format!("{}:\\", c as char))
        .filter(|d| std::path::Path::new(d).exists())
        .collect()
}

fn disk_space(drive: &str) -> (u64, u64) {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut total = 0u64;
    let mut free = 0u64;
    unsafe {
        let _ = GetDiskFreeSpaceExW(
            &HSTRING::from(drive),
            None,
            Some(&mut total),
            Some(&mut free),
        );
    }
    (total, free)
}

fn churn_color(churn: ChurnClass) -> Color32 {
    match churn {
        ChurnClass::Cold => Color32::from_rgb(100, 140, 210),
        ChurnClass::Hot => Color32::from_rgb(220, 100, 60),
        ChurnClass::Volatile => Color32::from_rgb(220, 190, 40),
    }
}

fn fmt_age(ts_secs: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let age = now.saturating_sub(ts_secs);
    if age < 3600 {
        format!("{}m ago", age / 60)
    } else if age < 86400 {
        format!("{}h ago", age / 3600)
    } else {
        format!("{}d ago", age / 86400)
    }
}

fn delta_sidebar_color(kind: &DeltaKind) -> Color32 {
    match kind {
        DeltaKind::New => Color32::from_rgb(80, 200, 100),
        DeltaKind::Grown => Color32::from_rgb(220, 100, 60),
        DeltaKind::Shrunk => Color32::from_rgb(100, 140, 210),
        DeltaKind::Deleted => Color32::from_rgb(140, 140, 140),
        DeltaKind::Unchanged => Color32::from_rgb(140, 140, 140),
    }
}

fn fmt_size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:>8.1} MB", bytes as f64 / 1e6)
    } else {
        format!("{:>8.0} KB", bytes as f64 / 1e3)
    }
}
