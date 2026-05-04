use std::sync::{Arc, Mutex};
use std::sync::mpsc::Receiver;

use eframe::egui;
use egui::{Color32, ComboBox, RichText, ScrollArea};

use crate::{
    classifier, entropy,
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
    selected_folder: Option<usize>,
    treemap_size: egui::Vec2,
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
        }
    }
}

impl eframe::App for DiskChurnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_rx(ctx);
        self.draw_toolbar(ctx);
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
                for file in snap.files.iter_mut() {
                    entropy::sample_entropy(file);
                }
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
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        scanner::scan(self.selected_drive.clone(), tx);
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
                            for d in self.drives.clone() {
                                ui.selectable_value(&mut self.selected_drive, d.clone(), d);
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
                        let active = self.filter_churn.as_ref() == Some(&class);
                        if ui.selectable_label(active, label).clicked() {
                            self.filter_churn = if active { None } else { Some(class) };
                            self.selected_folder = None;
                            self.dirty = true;
                        }
                    }
                }
            });
        });
    }

    fn draw_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar").min_width(220.0).show(ctx, |ui| {
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
                for (list_i, (_, folder)) in rows.iter().enumerate() {
                    let selected = self.selected_folder == Some(list_i);
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
                        self.selected_folder = if selected { None } else { Some(list_i) };
                    }
                }
            });
        });
    }

    fn draw_treemap(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let size = ui.available_size();
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
            let painter = ui.painter();
            let origin = ui.min_rect().min;
            treemap::paint(painter, &self.rects, &self.display_folders, origin);
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
