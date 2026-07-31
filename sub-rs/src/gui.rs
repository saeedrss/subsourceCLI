use crate::ads;
use crate::client::Client;
use crate::scan::{self, Stats};
use crate::updater;
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
    UpdateResult(Option<updater::UpdateInfo>),
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
    skip_existing: bool,
    recursive: bool,
    api_key: String,
    proxy: String,
    proxy_enabled: bool,
    files: Vec<FileRow>,
    log_text: String,
    stats_text: String,
    scanning: bool,
    show_global_log: bool,
    sel: Option<usize>,
    rx: Receiver<GuiEvent>,
    tx: Sender<GuiEvent>,
    lang_fa: bool,
    show_about: bool,
    show_update: bool,
    update_info: Option<updater::UpdateInfo>,
    prev_proxy: String,
    prev_proxy_enabled: bool,
    ad_data: Vec<ads::AdData>,
    ad_textures: Vec<egui::TextureHandle>,
    textures_loaded: bool,
    ads_signature: String,
    last_ads_check: f64,
    scan_completed_once: bool,
    show_ad_banner: bool,
    subtitle_lang: String,
}

impl SubGui {
    pub fn new(api_key: Option<String>, proxy: Option<String>, lang: &str, update: Option<updater::UpdateInfo>, ad_data: Vec<ads::AdData>) -> Self {
        let (tx, rx) = mpsc::channel();
        let show_update = update.is_some();
        SubGui {
            directory: String::new(),
            top_n: 5,
            dry_run: false,
            skip_existing: false,
            recursive: true,
            api_key: api_key.unwrap_or_default(),
            proxy: proxy.as_deref().unwrap_or("").to_string(),
            proxy_enabled: proxy.is_some() && proxy.as_deref().unwrap_or("") != "",
            files: Vec::new(),
            log_text: String::new(),
            stats_text: String::new(),
            scanning: false,
            show_global_log: true,
            sel: None,
            rx,
            tx,
            lang_fa: false,
            show_about: false,
            show_update,
            update_info: update,
            prev_proxy: String::new(),
            prev_proxy_enabled: false,
            ad_data,
            ad_textures: Vec::new(),
            textures_loaded: false,
            ads_signature: ads::ads_signature(),
            last_ads_check: 0.0,
            scan_completed_once: false,
            show_ad_banner: false,
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
        let skip_existing = self.skip_existing;
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
                let tx3 = tx.clone();
                let video_log: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
                let result = scan::process_video(video, &client, top_n, dry_run, &lang, skip_existing, &|msg| {
                    video_log.borrow_mut().push_str(msg);
                    tx2.send(GuiEvent::Log(msg.to_string())).ok();
                    tx3.send(GuiEvent::FileDetail(idx, video_log.borrow().clone())).ok();
                });

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

    fn reload_ads(&mut self) {
        self.ad_data = ads::load_ads();
        self.textures_loaded = false;
    }

    fn recheck_update(&self) {
        let tx = self.tx.clone();
        let proxy = if self.proxy_enabled && !self.proxy.is_empty() {
            Some(self.proxy.clone())
        } else {
            None
        };
        std::thread::spawn(move || {
            let result = updater::check_for_update(env!("CARGO_PKG_VERSION"), proxy.as_deref());
            tx.send(GuiEvent::UpdateResult(result)).ok();
        });
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
                GuiEvent::ScanDone => {
                    self.scanning = false;
                    self.scan_completed_once = true;
                    self.show_ad_banner = true;
                }
                GuiEvent::UpdateResult(info) => {
                    if let Some(ref u) = info {
                        self.log_text.push_str(&format!("\n[UPDATE] New version available: {}\n{}\n", u.latest_version, u.body));
                    }
                    self.update_info = info;
                    self.show_update = self.update_info.is_some();
                }
            }
            ctx.request_repaint();
        }

        if !self.textures_loaded {
            self.ad_textures.clear();
            if !self.ad_data.is_empty() {
                for (i, ad) in self.ad_data.iter().enumerate() {
                    if ad.width == 0 || ad.height == 0 {
                        continue;
                    }
                    let size = [ad.width as usize, ad.height as usize];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &ad.rgba);
                    let texture = ctx.load_texture(
                        format!("ad_{}", i),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.ad_textures.push(texture);
                }
            }
            self.textures_loaded = true;
        }

        let now = ctx.input(|i| i.time);
        if now - self.last_ads_check >= 3.0 {
            self.last_ads_check = now;
            let sig = ads::ads_signature();
            if sig != self.ads_signature {
                self.ads_signature = sig;
                self.reload_ads();
            }
        }


        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            if self.show_update {
                if let Some(ref info) = self.update_info.clone() {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(0, 100, 50))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("⬆ Update available: {} (current: v{})", info.latest_version, env!("CARGO_PKG_VERSION")));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("✕").clicked() {
                                        self.show_update = false;
                                    }
                                    if ui.button("View on GitHub").clicked() {
                                        let _ = webbrowser::open("https://github.com/saeedrss/subsourceCLI/releases/latest");
                                        self.show_update = false;
                                    }
                                });
                            });
                        });
                }
            }
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
                ui.checkbox(&mut self.skip_existing, if self.lang_fa { "رد فیلم‌های دارای زیرنویس" } else { "Skip Existing" });
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
                if self.prev_proxy != self.proxy || self.prev_proxy_enabled != self.proxy_enabled {
                    self.prev_proxy = self.proxy.clone();
                    self.prev_proxy_enabled = self.proxy_enabled;
                    self.recheck_update();
                    self.reload_ads();
                }
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

                    let log_selected = self.show_global_log;
                    let log_resp = ui.selectable_label(
                        log_selected,
                        if log_selected { "📋 Global Log" } else { "📋 Global Log" },
                    );
                    if log_resp.clicked() {
                        self.show_global_log = true;
                        self.sel = None;
                    }

                    if let Some(tex) = self.ad_textures.first() {
                        egui::TopBottomPanel::bottom("ad_panel").show_inside(ui, |ui| {
                            ui.separator();
                            ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(250.0, 80.0)));
                        });
                    }

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
                                self.show_global_log = false;
                            }
                        });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.show_ad_banner && self.ad_textures.len() > 1 {
                if let Some(tex) = self.ad_textures.get(1) {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(20, 20, 40))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(200.0, 60.0)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("✕").clicked() {
                                        self.show_ad_banner = false;
                                    }
                                });
                            });
                        });
                    ui.separator();
                }
            }

            if self.show_global_log || self.sel.is_none() {
                ui.label(if self.lang_fa { "لاگ کلی" } else { "Global Log" });
                ui.separator();
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
            } else if let Some(idx) = self.sel {
                if let Some(f) = self.files.get(idx) {
                    ui.label(&f.name);
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.label(&f.detail);
                        });
                    ui.separator();
                }
            }
        });

        if self.show_about {
            egui::Window::new("About").show(ctx, |ui| {
                ui.label("Subsource Farsi Subtitle Downloader");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
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
