mod ads;
mod clean;
mod client;
mod connect;
mod gui;
mod scan;
mod updater;

use crate::connect::{Connection, Mode};
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

    #[arg(long, value_enum)]
    connect: Option<Mode>,

    #[arg(long, help = "Cloudflare Worker URL (with --connect cloudflare)")]
    worker: Option<String>,

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

fn load_config() -> (Option<String>, Connection) {
    let env_key = std::env::var("SUBSOURCE_API_KEY").ok();
    let config_path = dirs::config_dir().map(|d| d.join("subsource").join("config.json"));
    let mut conn = Connection::direct();
    let mut file_key: Option<String> = None;
    if let Some(p) = config_path {
        if p.exists() {
            if let Ok(s) = std::fs::read_to_string(&p) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                    file_key = v.get("api_key").and_then(|k| k.as_str()).map(String::from);
                    let proxy = v
                        .get("proxy")
                        .and_then(|x| x.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from);
                    let worker = v
                        .get("worker")
                        .and_then(|x| x.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from);
                    let mode = match v.get("connect").and_then(|c| c.as_str()) {
                        Some("manual") => Some(Mode::Manual),
                        Some("system") => Some(Mode::System),
                        Some("cloudflare") => Some(Mode::Cloudflare),
                        _ => None,
                    };
                    conn = Connection::from_parts(mode, proxy, worker);
                }
            }
        }
    }
    (env_key.or(file_key), conn)
}

fn save_config(api_key: &str, conn: &Connection) -> Result<()> {
    let cfg_dir = dirs::config_dir()
        .ok_or_else(|| anyhow!("Cannot determine config directory"))?
        .join("subsource");
    std::fs::create_dir_all(&cfg_dir)?;
    let mode_str = match conn.mode {
        Mode::Direct => "direct",
        Mode::System => "system",
        Mode::Manual => "manual",
        Mode::Cloudflare => "cloudflare",
    };
    let mut map = serde_json::Map::new();
    map.insert("api_key".to_string(), serde_json::Value::String(api_key.to_string()));
    map.insert("connect".to_string(), serde_json::Value::String(mode_str.to_string()));
    if let Some(p) = &conn.manual_proxy {
        map.insert("proxy".to_string(), serde_json::Value::String(p.clone()));
    }
    if let Some(w) = &conn.worker_url {
        map.insert("worker".to_string(), serde_json::Value::String(w.clone()));
    }
    let json = serde_json::to_string_pretty(&serde_json::Value::Object(map))?;
    std::fs::write(cfg_dir.join("config.json"), json)?;
    Ok(())
}

fn resolve_connection(cli: &Cli, config_conn: Connection) -> Connection {
    let mut conn = config_conn;
    if let Some(m) = cli.connect {
        conn.mode = m;
    }
    if let Some(p) = &cli.proxy {
        conn.manual_proxy = Some(p.clone());
    }
    if let Some(w) = &cli.worker {
        conn.worker_url = Some(w.clone());
    }
    conn
}

fn run_cli(cli: &Cli) -> Result<()> {
    let (env_key, config_conn) = load_config();
    let api_key = cli
        .api_key
        .clone()
        .or(env_key)
        .unwrap_or_else(|| DEFAULT_API_KEY.to_string());
    let conn = resolve_connection(cli, config_conn);

    let client = client::Client::new(api_key.clone(), &conn)?;
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

    save_config(&api_key, &conn).ok();

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

    let (env_key, config_conn) = load_config();
    let conn = resolve_connection(&cli, config_conn);
    let update = updater::check_for_update(env!("CARGO_PKG_VERSION"), &conn);

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

        let app = gui::SubGui::new(api_key, conn, &cli.lang, update);
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
