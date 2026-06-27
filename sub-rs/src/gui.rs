use crate::client::Client;
use crate::scan::{self, Stats};
use eframe::egui;
use eframe::egui::Widget;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

#[derive(Clone)]
enum GuiEvent {
    Log(String),
    FileAdded(usize, String, String, String),
    FileUpdated(usize, String, String),
    FileDetail(usize, String),
    SetStats(String),
    ScanDone,
}

struct FileRow {
    name: String,
    ep: String,
    icon: String,
    status: String,
    detail: String,
}

pub struct SubGui {
    directory: String,
    top_n: usize,
    dry_run: bool,
    recursive: bool,
    api_key: String,
    proxy: String,
    proxy_enabled: bool,
    files: Vec<FileRow>,
    log_text: String,
    stats_text: String,
    scanning: bool,
    sel: Option<usize>,
    rx: Receiver<GuiEvent>,
    tx: Sender<GuiEvent>,
    lang_fa: bool,
    show_about: bool,
    subtitle_lang: String,
}

impl SubGui {
    pub fn new(api_key: Option<String>, proxy: Option<String>, lang: &str) -> Self {
        let (tx, rx) = mpsc::channel();
        SubGui {
            directory: String::new(),
            top_n: 5,
            dry_run: false,
            recursive: true,
            api_key: api_key.unwrap_or_default(),
            proxy: proxy.unwrap_or_default(),
            proxy_enabled: false,
            files: Vec::new(),
            log_text: String::new(),
            stats_text: String::new(),
            scanning: false,
            sel: None,
            rx,
            tx,
            lang_fa: false,
            show_about: false,
            subtitle_lang: lang.to_string(),
        }
    }

    fn start_scan(&mut self) {
        // ; pony: save config when user starts a scan
        if !self.api_key.is_empty() {
            let proxy_save = if self.proxy.is_empty() { None } else { Some(self.proxy.as_str()) };
            crate::save_config(&self.api_key, proxy_save).ok();
        }

        let api_key = self.api_key.clone();
        let proxy = if self.proxy_enabled && !self.proxy.is_empty() { Some(self.proxy.clone()) } else { None };
        let top_n = self.top_n;
        let dry_run = self.dry_run;
        let recursive = self.recursive;
        let dir = PathBuf::from(&self.directory);
        let tx = self.tx.clone();
        let lang = self.subtitle_lang.clone();

        self.files.clear();
        self.log_text.clear();
        self.stats_text.clear();
        self.sel = None;

        std::thread::spawn(move || {
            let client = match Client::new(api_key, proxy) {
                Ok(c) => c,
                Err(e) => {
                    tx.send(GuiEvent::Log(format!("[ERROR] Failed to create client: {}\n", e))).ok();
                    tx.send(GuiEvent::ScanDone).ok();
                    return;
                }
            };

            let mut idx = 0usize;
            let mut stats = Stats::new();
            let videos = scan::collect_videos(&dir, recursive);

            tx.send(GuiEvent::Log(format!("Found {} video file(s)\n", videos.len()))).ok();

            for video in &videos {
                let fname = video.file_name().unwrap_or_default().to_string_lossy().to_string();
                let fi = scan::parse_filename(&fname);
                let ep = if fi.is_episode {
                    format!("S{}E{}", fi.season.as_deref().unwrap_or(""), fi.episode.as_deref().unwrap_or(""))
                } else {
                    String::new()
                };
                tx.send(GuiEvent::FileAdded(idx, fname.clone(), ep, String::new())).ok();

                let tx2 = tx.clone();
                let video_log: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
                let result = scan::process_video(video, &client, top_n, dry_run, &lang, &|msg| {
                    video_log.borrow_mut().push_str(msg);
                    tx2.send(GuiEvent::Log(msg.to_string())).ok();
                });

                tx.send(GuiEvent::FileDetail(idx, video_log.take())).ok();

                match result {
                    Ok(true) => {
                        stats.found += 1;
                        stats.downloaded += 1;
                        tx.send(GuiEvent::FileUpdated(idx, "✅".to_string(), "Done".to_string())).ok();
                    }
                    Ok(false) => {
                        stats.errors += 1;
                        tx.send(GuiEvent::FileUpdated(idx, "❌".to_string(), "Failed".to_string())).ok();
                    }
                    Err(e) => {
                        stats.errors += 1;
                        tx.send(GuiEvent::Log(format!("  [ERROR] {}\n", e))).ok();
                        tx.send(GuiEvent::FileUpdated(idx, "❌".to_string(), "Error".to_string())).ok();
                    }
                }
                stats.scanned += 1;
                std::thread::sleep(std::time::Duration::from_millis(500));
                idx += 1;
            }

            let stats_str = format!(
                "Scanned: {} | Found: {} | Downloaded: {} | Errors: {} | Skipped: 0",
                stats.scanned, stats.found, stats.downloaded, stats.errors
            );
            tx.send(GuiEvent::SetStats(stats_str)).ok();
            tx.send(GuiEvent::ScanDone).ok();
        });

        self.scanning = true;
    }
}

impl eframe::App for SubGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                GuiEvent::Log(s) => self.log_text.push_str(&s),
                GuiEvent::FileAdded(_idx, name, ep, icon) => {
                    self.files.push(FileRow {
                        name,
                        ep,
                        icon,
                        status: "Pending".to_string(),
                        detail: String::new(),
                    });
                }
                GuiEvent::FileUpdated(idx, icon, status) => {
                    if let Some(f) = self.files.get_mut(idx) {
                        f.icon = icon;
                        f.status = status;
                    }
                }
                GuiEvent::FileDetail(idx, log) => {
                    if let Some(f) = self.files.get_mut(idx) {
                        f.detail = log;
                    }
                }
                GuiEvent::SetStats(s) => self.stats_text = s,
                GuiEvent::ScanDone => self.scanning = false,
            }
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button(if self.lang_fa { "انتخاب پوشه" } else { "Select Directory" })
                    .clicked()
                {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.directory = dir.to_string_lossy().to_string();
                    }
                }
                if ui.button(if self.lang_fa { "درباره" } else { "About" }).clicked() {
                    self.show_about = true;
                }
                if ui.button(if self.lang_fa { "خروج" } else { "Exit" }).clicked() {
                    std::process::exit(0);
                }
                ui.separator();
                ui.label(if self.lang_fa { "تعداد:" } else { "Top N:" });
                ui.add(egui::DragValue::new(&mut self.top_n).range(1..=50).speed(1));
                ui.checkbox(&mut self.dry_run, if self.lang_fa { "آزمایشی" } else { "Dry Run" });
                ui.checkbox(&mut self.recursive, if self.lang_fa { "به‌همراه زیرپوشه‌ها" } else { "Recursive" });
                ui.separator();
                ui.label(if self.lang_fa { "زیرنویس:" } else { "Sub:" });
                egui::ComboBox::from_id_salt("lang_selector")
                    .selected_text(&self.subtitle_lang)
                    .show_ui(ui, |ui| {
                        for (code, name, _) in scan::LANGUAGES {
                            ui.selectable_value(&mut self.subtitle_lang, code.to_string(), format!("{} — {}", code, name));
                        }
                    });
                if ui.button(if self.lang_fa { "شروع اسکن" } else { "Start Scan" })
                    .clicked()
                    && !self.scanning
                    && !self.directory.is_empty()
                {
                    self.start_scan();
                }
                ui.separator();
                if ui.button(if self.lang_fa { "EN" } else { "FA" }).clicked() {
                    self.lang_fa = !self.lang_fa;
                }
            });
            ui.horizontal(|ui| {
                ui.label(if self.lang_fa { "API Key:" } else { "API Key:" });
                ui.add(egui::TextEdit::singleline(&mut self.api_key).password(true).hint_text("sk_..."));
                ui.checkbox(&mut self.proxy_enabled, if self.lang_fa { "پروکسی" } else { "Proxy" });
                ui.add_enabled(self.proxy_enabled, egui::TextEdit::singleline(&mut self.proxy).hint_text("http://..."));
            });
            if !self.directory.is_empty() {
                ui.label(&self.directory);
            }
        });

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Made by ");
                ui.hyperlink_to("saeedrss", "https://github.com/saeedrss/subsourceCLI");
                ui.label("— Subsource subtitle downloader");
            });
        });

        egui::SidePanel::left("files_panel")
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(if self.lang_fa { "فایل‌های ویدیویی" } else { "Video Files" });
                    ui.separator();
                    ui.label(&self.stats_text);
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let mut clicked: Option<usize> = None;
                            for (i, f) in self.files.iter().enumerate() {
                                let resp = ui.selectable_label(
                                    self.sel == Some(i),
                                    format!("{} {}  {}  {}",
                                        f.icon,
                                        &f.name,
                                        if f.ep.is_empty() { "" } else { &f.ep },
                                        f.status
                                    ),
                                );
                                if resp.clicked() {
                                    clicked = Some(i);
                                }
                            }
                            if let Some(i) = clicked {
                                self.sel = Some(i);
                            }
                        });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(idx) = self.sel {
                if let Some(f) = self.files.get(idx) {
                    ui.label(&f.name);
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            egui::TextEdit::multiline(&mut f.detail.clone())
                                .desired_width(f32::INFINITY)
                                .desired_rows(10)
                                .ui(ui);
                        });
                    ui.separator();
                }
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let mut log = self.log_text.clone();
                    egui::TextEdit::multiline(&mut log)
                        .desired_width(f32::INFINITY)
                        .desired_rows(10)
                        .ui(ui);
                    self.log_text = log;
                });
        });

        if self.show_about {
            egui::Window::new("About").show(ctx, |ui| {
                ui.label("Subsource Farsi Subtitle Downloader");
                ui.label("Version 1.1.0");
                ui.separator();
                ui.label("Developed by:");
                ui.hyperlink_to("saeedrss", "https://github.com/saeedrss/subsourceCLI");
                ui.separator();
                if ui.button("Close").clicked() {
                    self.show_about = false;
                }
            });
        }
    }
}
