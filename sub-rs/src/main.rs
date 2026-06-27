mod client;
mod gui;
mod scan;

use anyhow::{anyhow, Result};
use clap::Parser;
use std::path::PathBuf;

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
        .ok_or_else(|| anyhow!("API key required. Use --api-key, SUBSOURCE_API_KEY env, or config file."))?;
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.gui || cli.directory.is_none() {
        let (env_key, file_proxy) = load_config();
        let api_key = cli.api_key.or(env_key);
        let proxy = cli.proxy.or(file_proxy);

        let options = eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 800.0])
                .with_resizable(true),
            ..Default::default()
        };

        let app = gui::SubGui::new(api_key, proxy, &cli.lang);
        eframe::run_native(
            "SubSource Subtitle Downloader",
            options,
            Box::new(|_cc| Ok(Box::new(app))),
        )
        .ok();
        Ok(())
    } else {
        run_cli(&cli)
    }
}
