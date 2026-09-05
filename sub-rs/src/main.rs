mod ads;
mod clean;
mod client;
mod gui;
mod scan;
mod updater;

use anyhow::{anyhow, Result};
use clap::Parser;
use std::path::PathBuf;

const DEFAULT_API_KEY: &str = "sk_f4963b83a6e4f6d5966c2d0fd31aa2b4b6e646e940142db2e04321e56e3881a6";

#[derive(Parser)]
#[command(name = "sub-rs", about = "Download subtitles via SubSource API")]
struct Cli {
    #[arg(short, long)]
    directory: Option<String>,

    #[arg(long, default_value = "5")]
    top: usize,

    #[arg(long)]
    api_key: Option<String>,

    #[arg(long)]
    no_recursive: bool,

    #[arg(long)]
    dry_run: bool,

    #[arg(long)]
    proxy: Option<String>,

    #[arg(short, long, default_value = "fa")]
    lang: String,

    #[arg(long)]
    skip_existing: bool,

    #[arg(long, help = "Save subtitle as <video>.srt without the language suffix (e.g. movie.srt instead of movie.fa.srt)")]
    no_lang_suffix: bool,

    #[arg(long, help = "Remove ad cues and brand watermarks from Farsi (.fa) subtitles after extraction")]
    clean_ads: bool,

    #[arg(long)]
    gui: bool,
}

fn load_config() -> (Option<String>, Option<String>) {
    let env_key = std::env::var("SUBSOURCE_API_KEY").ok();
    let config_path = dirs::config_dir().map(|d| d.join("subsource").join("config.json"));
    let (file_key, file_proxy) = match config_path {
        Some(p) if p.exists() => {
            std::fs::read_to_string(&p).ok().and_then(|s| {
                serde_json::from_str::<serde_json::Value>(&s).ok().map(|v| {
                    let key = v.get("api_key").and_then(|k| k.as_str()).map(String::from);
                    let proxy = v.get("proxy").and_then(|p| p.as_str()).map(String::from);
                    (key, proxy)
                })
            }).unwrap_or((None, None))
        }
        _ => (None, None),
    };
    (env_key.or(file_key), file_proxy)
}

fn save_config(api_key: &str, proxy: Option<&str>) -> Result<()> {
    let cfg_dir = dirs::config_dir()
        .ok_or_else(|| anyhow!("Cannot determine config directory"))?
        .join("subsource");
    std::fs::create_dir_all(&cfg_dir)?;
    let mut map = serde_json::Map::new();
    map.insert("api_key".to_string(), serde_json::Value::String(api_key.to_string()));
    if let Some(p) = proxy {
        if !p.is_empty() {
            map.insert("proxy".to_string(), serde_json::Value::String(p.to_string()));
        }
    }
    let json = serde_json::to_string_pretty(&serde_json::Value::Object(map))?;
    std::fs::write(cfg_dir.join("config.json"), json)?;
    Ok(())
}

fn run_cli(cli: &Cli) -> Result<()> {
    let (env_key, file_proxy) = load_config();
    let api_key = cli
        .api_key
        .clone()
        .or(env_key)
        .unwrap_or_else(|| DEFAULT_API_KEY.to_string());
    let proxy = cli.proxy.clone().or(file_proxy);

    let client = client::Client::new(api_key.clone(), proxy.clone())?;
    let dir = PathBuf::from(cli.directory.as_deref().unwrap_or("."));
    if !dir.exists() {
        anyhow::bail!("Directory not found: {}", dir.display());
    }

    let stats = scan::scan_directory(
        &dir,
        &client,
        cli.top,
        !cli.no_recursive,
        cli.dry_run,
        &cli.lang,
        cli.skip_existing,
        cli.no_lang_suffix,
        cli.clean_ads,
        &|msg| print!("{}", msg),
    )?;

    println!("\n{}", "=".repeat(70));
    println!("[STATS] FINAL STATISTICS");
    println!("{}", "=".repeat(70));
    println!("  Scanned:    {}", stats.scanned);
    println!("  Found:      {}", stats.found);
    println!("  Downloaded: {}", stats.downloaded);
    println!("  Skipped:    {}", stats.skipped);
    println!("  Errors:     {}", stats.errors);
    println!("{}", "=".repeat(70));

    // ; pony: save config after successful run
    let proxy_save = if cli.proxy.is_some() { cli.proxy.as_deref() } else { None };
    save_config(&api_key, proxy_save).ok();

    Ok(())
}

#[cfg(windows)]
fn hide_console() {
    extern "system" {
        fn FreeConsole() -> i32;
    }
    unsafe { FreeConsole(); }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (env_key, file_proxy) = load_config();
    let proxy = cli.proxy.clone().or(file_proxy);
    let update = updater::check_for_update(env!("CARGO_PKG_VERSION"), proxy.as_deref());

    if cli.gui || cli.directory.is_none() {
        #[cfg(windows)]
        hide_console();

        let api_key = cli.api_key.or(env_key).or(Some(DEFAULT_API_KEY.to_string()));

        let mut viewport = eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_resizable(true);
        if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
            viewport = viewport.with_icon(icon);
        }

        let options = eframe::NativeOptions {
            viewport,
            ..Default::default()
        };

        let app = gui::SubGui::new(api_key, proxy, &cli.lang, update);
        eframe::run_native(
            "SubSource Subtitle Downloader",
            options,
            Box::new(|cc| {
                gui::install_fonts(&cc.egui_ctx);
                Ok(Box::new(app))
            }),
        )
        .ok();
        Ok(())
    } else {
        if let Some(u) = &update {
            println!("\n{}", "=".repeat(70));
            println!("[UPDATE] New version available: {} (current: v{})", u.latest_version, env!("CARGO_PKG_VERSION"));
            println!("{}", "=".repeat(70));
            println!("{}", u.body);
            println!("{}", "=".repeat(70));
        }
        run_cli(&cli)
    }
}
