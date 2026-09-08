# Connection Methods Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let sub-rs reach the SubSource API via Direct, System proxy, Manual proxy, or a Cloudflare Worker — verifying the Worker route works before shipping.

**Architecture:** A new `src/connect.rs` module defines a `Mode` enum and `Connection` struct used by the API client, ads loader, and update checker. All four modes configure a single reqwest builder; Cloudflare mode swaps the API base URL to a transparent reverse-proxy Worker (`cloudflare-worker/`) deployed by the user. CLI and GUI both expose the mode selection; config persists it.

**Tech Stack:** Rust (clap derive, serde/serde_json, reqwest blocking), egui 0.30, JS Worker (workers.dev), wrangler.

**Spec:** `docs/superpowers/specs/2026-09-08-connection-methods-design.md`

**Checkpoint discipline:** This plan stops after **Task 6** for a manual end-to-end test through the deployed Worker. Tasks 7–9 (GUI + docs + release) run ONLY if that test succeeds.

---

### Task 1: Cloudflare Worker script

**Files:**
- Create: `cloudflare-worker/worker.js`
- Create: `cloudflare-worker/wrangler.toml`

- [ ] **Step 1: Write `cloudflare-worker/worker.js`**

```js
const UPSTREAM = "https://api.subsource.net";

export default {
  async fetch(request) {
    const url = new URL(request.url);
    const target = UPSTREAM + url.pathname + url.search;

    const headers = new Headers(request.headers);
    headers.delete("host");

    const init = { method: request.method, headers };
    if (request.method !== "GET" && request.method !== "HEAD") {
      init.body = request.body;
    }

    return fetch(target, init);
  },
};
```

- [ ] **Step 2: Write `cloudflare-worker/wrangler.toml`**

```toml
name = "subsource-proxy"
main = "worker.js"
compatibility_date = "2025-01-01"
```

- [ ] **Step 3: Syntax-check the JS**

Run:
```bash
node --check cloudflare-worker/worker.js
```
Expected: exit 0, no output (skip if node is not installed).

- [ ] **Step 4: Commit**

```bash
git add cloudflare-worker/worker.js cloudflare-worker/wrangler.toml
git commit -m "Add Cloudflare worker reverse proxy for SubSource API"
```

---

### Task 2: `src/connect.rs` (typed connection model)

**Files:**
- Create: `src/connect.rs`
- Modify: `src/main.rs:1-7` (add `mod connect;`)

- [ ] **Step 1: Write `src/connect.rs`**

```rust
use anyhow::{anyhow, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const API_BASE: &str = "https://api.subsource.net/api/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum Mode {
    Direct,
    System,
    Manual,
    Cloudflare,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub mode: Mode,
    pub manual_proxy: Option<String>,
    pub worker_url: Option<String>,
}

impl Connection {
    pub fn direct() -> Self {
        Connection {
            mode: Mode::Direct,
            manual_proxy: None,
            worker_url: None,
        }
    }

    pub fn from_parts(
        mode: Option<Mode>,
        manual_proxy: Option<String>,
        worker_url: Option<String>,
    ) -> Self {
        let mode = match mode {
            Some(m) => m,
            None if manual_proxy.is_some() => Mode::Manual,
            None => Mode::Direct,
        };
        Connection {
            mode,
            manual_proxy,
            worker_url,
        }
    }

    pub fn api_base(&self) -> String {
        match self.mode {
            Mode::Cloudflare => match &self.worker_url {
                Some(w) => format!("{}/api/v1", w.trim_end_matches('/')),
                None => API_BASE.to_string(),
            },
            _ => API_BASE.to_string(),
        }
    }

    pub fn builder(&self) -> Result<reqwest::blocking::ClientBuilder> {
        let mut builder = reqwest::blocking::ClientBuilder::new();
        match self.mode {
            Mode::Direct | Mode::Cloudflare => {
                builder = builder.no_proxy();
            }
            Mode::System => {}
            Mode::Manual => {
                let url = self
                    .manual_proxy
                    .as_deref()
                    .ok_or_else(|| anyhow!("Manual proxy selected but no proxy URL configured"))?;
                builder = builder.proxy(reqwest::Proxy::all(url)?);
            }
        }
        Ok(builder)
    }

    pub fn build_reqwest(
        &self,
        timeout: Duration,
        user_agent: &str,
    ) -> Result<reqwest::blocking::Client> {
        Ok(self.builder()?.timeout(timeout).user_agent(user_agent).build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ValueEnum;

    #[test]
    fn mode_parses_from_clap_lowercase() {
        assert_eq!(Mode::from_str("direct", true).unwrap(), Mode::Direct);
        assert_eq!(Mode::from_str("system", true).unwrap(), Mode::System);
        assert_eq!(Mode::from_str("manual", true).unwrap(), Mode::Manual);
        assert_eq!(
            Mode::from_str("cloudflare", true).unwrap(),
            Mode::Cloudflare
        );
    }

    #[test]
    fn mode_parses_from_serde_lowercase() {
        assert_eq!(
            serde_json::from_str::<Mode>("\"manual\"").unwrap(),
            Mode::Manual
        );
        assert_eq!(
            serde_json::from_str::<Mode>("\"cloudflare\"").unwrap(),
            Mode::Cloudflare
        );
    }

    #[test]
    fn from_parts_defaults_to_direct() {
        let c = Connection::from_parts(None, None, None);
        assert_eq!(c.mode, Mode::Direct);
    }

    #[test]
    fn from_parts_proxy_implies_manual() {
        let c = Connection::from_parts(None, Some("http://1.2.3.4:8080".into()), None);
        assert_eq!(c.mode, Mode::Manual);
        assert_eq!(c.manual_proxy.as_deref(), Some("http://1.2.3.4:8080"));
    }

    #[test]
    fn from_parts_explicit_mode_wins() {
        let c = Connection::from_parts(Some(Mode::System), Some("http://1.2.3.4:8080".into()), None);
        assert_eq!(c.mode, Mode::System);
    }

    #[test]
    fn api_base_cloudflare_uses_worker() {
        let c = Connection::from_parts(
            Some(Mode::Cloudflare),
            None,
            Some("https://subsource-proxy.workers.dev".into()),
        );
        assert_eq!(c.api_base(), "https://subsource-proxy.workers.dev/api/v1");
    }

    #[test]
    fn api_base_trailing_slash_trimmed() {
        let c = Connection::from_parts(
            Some(Mode::Cloudflare),
            None,
            Some("https://subsource-proxy.workers.dev/".into()),
        );
        assert_eq!(c.api_base(), "https://subsource-proxy.workers.dev/api/v1");
    }

    #[test]
    fn api_base_direct_uses_subsource() {
        let c = Connection::direct();
        assert_eq!(c.api_base(), API_BASE);
    }

    #[test]
    fn build_reqwest_manual_without_proxy_errors() {
        let c = Connection::from_parts(Some(Mode::Manual), None, None);
        assert!(c.build_reqwest(Duration::from_secs(5), "test").is_err());
    }

    #[test]
    fn build_reqwest_direct_builds_client() {
        let c = Connection::direct();
        assert!(c.build_reqwest(Duration::from_secs(5), "test").is_ok());
    }
}
```

- [ ] **Step 2: Register the module in `src/main.rs`**

Change:
```rust
mod ads;
mod clean;
mod client;
mod gui;
mod scan;
mod updater;
```
to:
```rust
mod ads;
mod clean;
mod client;
mod connect;
mod gui;
mod scan;
mod updater;
```
And add `use` (top of file, after the module decls):
```rust
use crate::connect::{Connection, Mode};
```

- [ ] **Step 3: Run the new tests**

Run: `cargo test connect --lib`
Expected: 11 passed, 0 failed.

- [ ] **Step 4: Commit**

```bash
git add src/connect.rs src/main.rs
git commit -m "Add typed connection model (direct/system/manual/cloudflare)"
```

---

### Task 3: Refactor `client.rs` to use `Connection`

**Files:**
- Modify: `src/client.rs`

- [ ] **Step 1: Replace the module header**

Change:
```rust
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

const API_BASE: &str = "https://api.subsource.net/api/v1";
const REQUEST_DELAY_SECS: f64 = 1.0;
```
to:
```rust
use crate::connect::Connection;
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

const REQUEST_DELAY_SECS: f64 = 1.0;
```

- [ ] **Step 2: Add `api_base` to the Client struct**

Change:
```rust
pub struct Client {
    http: reqwest::blocking::Client,
    api_key: String,
}
```
to:
```rust
pub struct Client {
    http: reqwest::blocking::Client,
    api_key: String,
    api_base: String,
}
```

- [ ] **Step 3: Rewrite `Client::new`**

Change:
```rust
    pub fn new(api_key: String, proxy: Option<String>) -> Result<Self> {
        let mut builder = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
        if let Some(p) = proxy {
            builder = builder.proxy(reqwest::Proxy::all(&p)?);
        }
        Ok(Client {
            http: builder.build()?,
            api_key,
        })
    }
```
to:
```rust
    pub fn new(api_key: String, conn: &Connection) -> Result<Self> {
        Ok(Client {
            http: conn.build_reqwest(
                Duration::from_secs(30),
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )?,
            api_key,
            api_base: conn.api_base(),
        })
    }
```

- [ ] **Step 4: Use `self.api_base` in the three endpoint calls**

Change all three occurrences of `API_BASE`:
```rust
.get(format!("{}/movies/search", API_BASE))
```
→
```rust
.get(format!("{}/movies/search", self.api_base))
```
```rust
.get(format!("{}/subtitles", API_BASE))
```
→
```rust
.get(format!("{}/subtitles", self.api_base))
```
```rust
.get(format!("{}/subtitles/{}/download", API_BASE, subtitle_id))
```
→
```rust
.get(format!("{}/subtitles/{}/download", self.api_base, subtitle_id))
```

- [ ] **Step 5: Verify compile**

Run: `cargo build`
Expected: `src/connect.rs` compiles; the only errors are in `gui.rs` (and later `main.rs`) where `Client::new` is still called with `Option<String>`. Those are fixed in Tasks 5 and 7.

- [ ] **Step 6: Commit**

```bash
git add src/client.rs
git commit -m "Use Connection model in SubSource API client"
```

---

### Task 4: Refactor `ads.rs` and `updater.rs` to use `Connection`

**Files:**
- Modify: `src/ads.rs`
- Modify: `src/updater.rs`

- [ ] **Step 1: Update `src/ads.rs`**

Change:
```rust
use serde::Deserialize;
```
to:
```rust
use crate::connect::Connection;
use serde::Deserialize;
use std::time::Duration;
```

Change:
```rust
pub fn fetch_ads(proxy: Option<&str>) -> Vec<AdData> {
    let client = http_client(proxy);
```
to:
```rust
pub fn fetch_ads(conn: &Connection) -> Vec<AdData> {
    let client = match conn.build_reqwest(Duration::from_secs(10), "sub-rs") {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
```

Delete the `http_client` function entirely (lines ~50–62):
```rust
fn http_client(proxy: Option<&str>) -> reqwest::blocking::Client {
    let mut builder = reqwest::blocking::Client::builder()
        .user_agent("sub-rs")
        .timeout(std::time::Duration::from_secs(10));
    if let Some(p) = proxy {
        if !p.is_empty() {
            if let Ok(proxy) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy);
            }
        }
    }
    builder.build().unwrap_or_else(|_| reqwest::blocking::Client::new())
}
```

- [ ] **Step 2: Update `src/updater.rs`**

Change:
```rust
use serde::Deserialize;
```
to:
```rust
use crate::connect::Connection;
use serde::Deserialize;
use std::time::Duration;
```

Change:
```rust
pub fn check_for_update(current: &str, proxy: Option<&str>) -> Option<UpdateInfo> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .user_agent("sub-rs/1.0");
    if let Some(p) = proxy {
        if !p.is_empty() {
            if let Ok(proxy) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy);
            }
        }
    }
    let client = match builder.build() {
        Ok(c) => c,
        Err(_) => return None,
    };
```
to:
```rust
pub fn check_for_update(current: &str, conn: &Connection) -> Option<UpdateInfo> {
    let client = match conn.build_reqwest(Duration::from_secs(TIMEOUT_SECS), "sub-rs/1.0") {
        Ok(c) => c,
        Err(_) => return None,
    };
```

- [ ] **Step 3: Verify compile**

Run: `cargo build`
Expected: compile errors only in `main.rs` and `gui.rs` (callers still use `Option<String>`); those are fixed in Tasks 5 and 7.

- [ ] **Step 4: Commit**

```bash
git add src/ads.rs src/updater.rs
git commit -m "Use Connection for ads and update checker HTTP clients"
```

---

### Task 5: CLI flag, config load/save, and routing in `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add CLI flags**

After the existing `proxy` field in the `Cli` struct:
```rust
    #[arg(long)]
    proxy: Option<String>,
```
add:
```rust
    #[arg(long, value_enum)]
    connect: Option<Mode>,

    #[arg(long, help = "Cloudflare Worker URL (requires --connect cloudflare)")]
    worker: Option<String>,
```

- [ ] **Step 2: Rewrite `load_config`**

Replace the whole `load_config` function:
```rust
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
```
with:
```rust
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
```

- [ ] **Step 3: Rewrite `save_config`**

Replace:
```rust
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
```
with:
```rust
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
```

- [ ] **Step 4: Add a `resolve_connection` helper**

Add after `save_config`:
```rust
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
```

- [ ] **Step 5: Update `run_cli`**

Replace:
```rust
    let (env_key, file_proxy) = load_config();
    let api_key = cli
        .api_key
        .clone()
        .or(env_key)
        .unwrap_or_else(|| DEFAULT_API_KEY.to_string());
    let proxy = cli.proxy.clone().or(file_proxy);

    let client = client::Client::new(api_key.clone(), proxy.clone())?;
```
with:
```rust
    let (env_key, config_conn) = load_config();
    let api_key = cli
        .api_key
        .clone()
        .or(env_key)
        .unwrap_or_else(|| DEFAULT_API_KEY.to_string());
    let conn = resolve_connection(cli, config_conn);

    let client = client::Client::new(api_key.clone(), &conn)?;
```

And the `save_config` call at the end of `run_cli`:
```rust
    let proxy_save = if cli.proxy.is_some() { cli.proxy.as_deref() } else { None };
    save_config(&api_key, proxy_save).ok();
```
becomes:
```rust
    save_config(&api_key, &conn).ok();
```

- [ ] **Step 6: Update `main()`**

Replace:
```rust
    let (env_key, file_proxy) = load_config();
    let proxy = cli.proxy.clone().or(file_proxy);
    let update = updater::check_for_update(env!("CARGO_PKG_VERSION"), proxy.as_deref());
```
with:
```rust
    let (env_key, config_conn) = load_config();
    let conn = resolve_connection(&cli, config_conn);
    let update = updater::check_for_update(env!("CARGO_PKG_VERSION"), &conn);
```

And the GUI spawn:
```rust
        let app = gui::SubGui::new(api_key, proxy, &cli.lang, update);
```
becomes:
```rust
        let app = gui::SubGui::new(api_key, conn, &cli.lang, update);
```

- [ ] **Step 7: Build and test**

Run: `cargo test`
Expected: failure only in compile of `gui.rs` (SubGui::new signature changed, `proxy` param removed). Confirm `connect::tests` still pass (they run first). Fix is Task 7.

- [ ] **Step 8: Commit**

```bash
git add src/main.rs
git commit -m "Add --connect/--worker flags; persist connection mode in config"
```

---

### Task 6: Cloudflare end-to-end checkpoint (STOP HERE)

**Purpose:** Prove the Worker route works against the real API before any GUI work.

- [ ] **Step 1: Build a release binary**

Run (in `sub-rs/`):
```bash
cargo build --release
```
Expected: `Finished release profile` with a `target/release/sub-rs.exe`.

- [ ] **Step 2: User deploys the worker**

In a terminal on the user's machine (with Cloudflare `wrangler` authenticated):
```bash
cd cloudflare-worker
wrangler deploy
```
Or paste `worker.js` into the Cloudflare Dashboard → Workers & Pages → Create Worker → paste → Deploy.
Note the resulting URL, e.g. `https://subsource-proxy.<your-subdomain>.workers.dev`.

- [ ] **Step 3: Verify the worker directly (raw HTTP)**

Run in PowerShell (replace `<WORKER_URL>` and the API key):
```powershell
$r = Invoke-RestMethod -Uri "<WORKER_URL>/api/v1/movies/search?searchType=text&type=all&q=Interstellar" -Headers @{ "X-API-Key" = "sk_f4963b83a6e4f6d5966c2d0fd31aa2b4b6e646e940142db2e04321e56e3881a6" }
$r | ConvertTo-Json -Depth 4
```
Expected: a JSON array of movie objects (`movieId`, `title`...). If the request errors/slow-times-out, the Worker or ISP path is broken — STOP and debug the Worker before continuing.

- [ ] **Step 4: Verify through the real CLI (dry run)**

Create a test folder with one known movie file, then run:
```
cd sub-rs
./target/release/sub-rs --connect cloudflare --worker "<WORKER_URL>" --dry-run --directory "G:\1080\TestFolder"
```
Expected: logs show the movie match and subtitle list in the log with **no network/connection errors**.

- [ ] **Step 5: Manual approval gate**

Ask the user: **"Cloudflare method works — proceed with the GUI, docs, and v1.1.5 release?"**
- Yes → continue to Task 7.
- No → debug the Worker path, do not proceed.

---

### Task 7: GUI radio buttons + `Connection` threading

**Files:**
- Modify: `src/main.rs` (already done in Task 5)
- Modify: `src/gui.rs`

- [ ] **Step 1: Update imports**

Change:
```rust
use crate::ads;
use crate::client::Client;
use crate::scan::{self, Stats};
use crate::updater;
```
to:
```rust
use crate::ads;
use crate::client::Client;
use crate::connect::{Connection, Mode};
use crate::scan::{self, Stats};
use crate::updater;
```

- [ ] **Step 2: Replace proxy fields in `SubGui`**

Change:
```rust
    api_key: String,
    proxy: String,
    proxy_enabled: bool,
    clean_ads: bool,
```
to:
```rust
    api_key: String,
    mode: Mode,
    manual_proxy: String,
    worker_url: String,
    clean_ads: bool,
```

And change:
```rust
    prev_proxy: String,
    prev_proxy_enabled: bool,
```
to:
```rust
    prev_conn: Connection,
```

- [ ] **Step 3: Update `SubGui::new`**

Change the signature:
```rust
    pub fn new(api_key: Option<String>, proxy: Option<String>, lang: &str, update: Option<updater::UpdateInfo>) -> Self {
```
to:
```rust
    pub fn new(api_key: Option<String>, conn: Connection, lang: &str, update: Option<updater::UpdateInfo>) -> Self {
```

Replace the ads-fetch spawn (uses `fetch_proxy`):
```rust
        let fetch_tx = tx.clone();
        let fetch_proxy = proxy.clone();
        std::thread::spawn(move || {
            let ads = ads::fetch_ads(fetch_proxy.as_deref());
            fetch_tx.send(GuiEvent::AdsLoaded(ads)).ok();
        });
```
with:
```rust
        let fetch_tx = tx.clone();
        let fetch_conn = conn.clone();
        std::thread::spawn(move || {
            let ads = ads::fetch_ads(&fetch_conn);
            fetch_tx.send(GuiEvent::AdsLoaded(ads)).ok();
        });
```

Replace the field initializers:
```rust
            api_key: api_key.unwrap_or_default(),
            proxy: proxy.as_deref().unwrap_or("").to_string(),
            proxy_enabled: false,
            clean_ads: false,
```
with:
```rust
            api_key: api_key.unwrap_or_default(),
            mode: conn.mode,
            manual_proxy: conn.manual_proxy.clone().unwrap_or_default(),
            worker_url: conn.worker_url.clone().unwrap_or_default(),
            clean_ads: false,
```

And change:
```rust
            prev_proxy: String::new(),
            prev_proxy_enabled: false,
```
to:
```rust
            prev_conn: conn,
```

- [ ] **Step 4: Add `current_connection` helper**

Add after `SubGui::new`:
```rust
    fn current_connection(&self) -> Connection {
        Connection {
            mode: self.mode,
            manual_proxy: if self.manual_proxy.is_empty() {
                None
            } else {
                Some(self.manual_proxy.clone())
            },
            worker_url: if self.worker_url.is_empty() {
                None
            } else {
                Some(self.worker_url.clone())
            },
        }
    }
```

- [ ] **Step 5: Update `start_scan`**

Replace:
```rust
        if !self.api_key.is_empty() {
            let proxy_save = if self.proxy.is_empty() { None } else { Some(self.proxy.as_str()) };
            crate::save_config(&self.api_key, proxy_save).ok();
        }

        let api_key = self.api_key.clone();
        let proxy = if self.proxy_enabled && !self.proxy.is_empty() { Some(self.proxy.clone()) } else { None };
        let top_n = self.top_n;
```
with:
```rust
        let conn = self.current_connection();
        if !self.api_key.is_empty() {
            crate::save_config(&self.api_key, &conn).ok();
        }

        let api_key = self.api_key.clone();
        let conn_clone = conn.clone();
        let top_n = self.top_n;
```

And in the background thread, change:
```rust
            let client = match Client::new(api_key, proxy) {
```
to:
```rust
            let client = match Client::new(api_key, &conn_clone) {
```

- [ ] **Step 6: Update `reload_ads` and `recheck_update`**

Replace `reload_ads`:
```rust
    fn reload_ads(&self) {
        let tx = self.tx.clone();
        let proxy = if self.proxy_enabled && !self.proxy.is_empty() {
            Some(self.proxy.clone())
        } else {
            None
        };
        std::thread::spawn(move || {
            let ads = ads::fetch_ads(proxy.as_deref());
            tx.send(GuiEvent::AdsLoaded(ads)).ok();
        });
    }
```
with:
```rust
    fn reload_ads(&self) {
        let tx = self.tx.clone();
        let conn = self.current_connection();
        std::thread::spawn(move || {
            let ads = ads::fetch_ads(&conn);
            tx.send(GuiEvent::AdsLoaded(ads)).ok();
        });
    }
```

Replace `recheck_update`:
```rust
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
```
with:
```rust
    fn recheck_update(&self) {
        let tx = self.tx.clone();
        let conn = self.current_connection();
        std::thread::spawn(move || {
            let result = updater::check_for_update(env!("CARGO_PKG_VERSION"), &conn);
            tx.send(GuiEvent::UpdateResult(result)).ok();
        });
    }
```

- [ ] **Step 7: Replace the toolbar proxy UI**

Replace the second `ui.horizontal` proxy block (currently lines ~372–384):
```rust
            ui.horizontal(|ui| {
                ui.label(if self.lang_fa { "API Key:" } else { "API Key:" });
                ui.add(egui::TextEdit::singleline(&mut self.api_key).password(true).hint_text("sk_..."));
                ui.checkbox(&mut self.proxy_enabled, if self.lang_fa { "پروکسی" } else { "Proxy" });
                ui.add_enabled(self.proxy_enabled, egui::TextEdit::singleline(&mut self.proxy).hint_text("http://ip:port / socks5://ip:port"));
                ui.checkbox(&mut self.clean_ads, if self.lang_fa { "حذف تبلیغات فارسی" } else { "Remove Farsi ads" });
                if self.prev_proxy != self.proxy || self.prev_proxy_enabled != self.proxy_enabled {
                    self.prev_proxy = self.proxy.clone();
                    self.prev_proxy_enabled = self.proxy_enabled;
                    self.recheck_update();
                    self.reload_ads();
                }
            });
```
with:
```rust
            ui.horizontal(|ui| {
                ui.label(if self.lang_fa { "API Key:" } else { "API Key:" });
                ui.add(egui::TextEdit::singleline(&mut self.api_key).password(true).hint_text("sk_..."));
                ui.checkbox(&mut self.clean_ads, if self.lang_fa { "حذف تبلیغات فارسی" } else { "Remove Farsi ads" });
            });
            ui.horizontal(|ui| {
                ui.label(if self.lang_fa { "اتصال:" } else { "Connect:" });
                ui.radio_value(&mut self.mode, Mode::Direct, if self.lang_fa { "مستقیم" } else { "Direct" });
                ui.radio_value(&mut self.mode, Mode::System, if self.lang_fa { "سیستم" } else { "System proxy" });
                ui.radio_value(&mut self.mode, Mode::Manual, if self.lang_fa { "پروکسی دستی" } else { "Manual proxy" });
                ui.radio_value(&mut self.mode, Mode::Cloudflare, if self.lang_fa { "کلادفلر" } else { "Cloudflare" });
                if self.mode == Mode::Manual {
                    ui.add_enabled(true, egui::TextEdit::singleline(&mut self.manual_proxy).hint_text("http://ip:port / socks5://ip:port"));
                }
                if self.mode == Mode::Cloudflare {
                    ui.add_enabled(true, egui::TextEdit::singleline(&mut self.worker_url).hint_text("https://<name>.workers.dev"));
                }
                let cur = self.current_connection();
                if self.prev_conn != cur {
                    self.prev_conn = cur;
                    self.recheck_update();
                    self.reload_ads();
                }
            });
```

- [ ] **Step 8: Build and test**

Run: `cargo test`
Expected: all tests pass (connect 11 + clean 11), no compile errors.

- [ ] **Step 9: Run the GUI once** (manual smoke test)

Run: `cargo run -- --gui`
Expected: window opens with the four radios; selecting Manual shows the proxy textbox, selecting Cloudflare shows the worker textbox, Direct/System show no textbox. Closing returns to shell.

- [ ] **Step 10: Commit**

```bash
git add src/gui.rs src/main.rs
git commit -m "GUI: connection-mode radio buttons (direct/system/manual/cloudflare)"
```

---

### Task 8: Docs — README (en + fa) and CHANGELOG

**Files:**
- Modify: `README.md`
- Modify: `faREADME.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: `README.md` — add CLI rows**

In the CLI options table, after the `--proxy` row:
```markdown
| `--proxy` | `None` | Proxy URL (e.g. `http://127.0.0.1:8080`) |
```
add:
```markdown
| `--connect` | `direct` | Connection method: `direct`, `system`, `manual`, `cloudflare` |
| `--worker` | `None` | Cloudflare Worker URL (with `--connect cloudflare`) |
```

- [ ] **Step 2: `README.md` — add "Connection methods" section**

Add after the "Output layout" section (between the output code block and the `## ✨ Features` heading):
```markdown
## 🔌 Connection methods

The app can reach the SubSource API four ways (CLI `--connect`, or the radio
buttons in the GUI):

- **Direct** — no proxy at all.
- **System proxy** — use the OS/environment proxy automatically.
- **Manual proxy** — a proxy URL you type in (e.g. `http://ip:port`,
  `socks5://ip:port`).
- **Cloudflare** — route through a [Cloudflare Worker](cloudflare-worker/)
  that forwards to `api.subsource.net`, useful when your ISP blocks or
  throttles the API host directly.

```bash
# direct (default)
./sub-rs --directory "/path/to/videos"
# manual proxy
./sub-rs --connect manual --proxy "http://127.0.0.1:8080" --directory "/path/to/videos"
# cloudflare worker
./sub-rs --connect cloudflare --worker "https://subsource-proxy.example.workers.dev" --directory "/path/to/videos"
```
```

- [ ] **Step 3: `faREADME.md` — add CLI rows**

In the fa options table, after the `--proxy` row:
```markdown
| `--proxy` | — | آدرس پروکسی |
```
add:
```markdown
| `--connect` | `direct` | روش اتصال: `direct`، `system`، `manual`، `cloudflare` |
| `--worker` | — | آدرس Worker کلادفلر (با `--connect cloudflare`) |
```

- [ ] **Step 4: `faREADME.md` — add "روش‌های اتصال" section**

Add after the "رابط گرافیکی (GUI)" section, before "## زبان های پشتیبانی شده":
```markdown
## روش‌های اتصال

برنامه با چهار روش به API متصل می‌شود (در GUI با دکمه‌های رادیویی، در CLI با
`--connect`):

- **مستقیم** — بدون پروکسی
- **پروکسی سیستم** — استفاده خودکار از پروکسی سیستم/محیط
- **پروکسی دستی** — آدرس پروکسی که خودتان وارد می‌کنید
- **کلادفلر** — عبور از طریق
  [Cloudflare Worker](cloudflare-worker/) که به
  `api.subsource.net` فوروارد می‌کند؛ برای وقتی که ISP دسترسی مستقیم به API
  را مسدود یا محدود کرده است

```bash
# مستقیم (پیش‌فرض)
./sub-rs --directory "/path/to/videos"
# پروکسی دستی
./sub-rs --connect manual --proxy "http://127.0.0.1:8080" --directory "/path/to/videos"
# کلادفلر
./sub-rs --connect cloudflare --worker "https://subsource-proxy.example.workers.dev" --directory "/path/to/videos"
```
```

- [ ] **Step 5: `CHANGELOG.md` — add 1.1.5 entry**

At the top, after `# Changelog`:
```markdown
## [1.1.5] - 2026-09-08

### Added
- Connection methods: Direct / System proxy / Manual proxy / Cloudflare Worker
  (`--connect` + `--worker` CLI flags, GUI radio buttons, persisted in config)
- Cloudflare Worker reverse proxy (`cloudflare-worker/`) to bypass ISP blocking
  of `api.subsource.net`
```

- [ ] **Step 6: Commit**

```bash
git add README.md faREADME.md CHANGELOG.md
git commit -m "Document connection methods (direct/system/manual/cloudflare)"
```

---

### Task 9: Version bump 1.1.5, final build, tag, push

**Files:**
- Modify: `sub-rs/Cargo.toml`

- [ ] **Step 1: Bump version**

Change in `sub-rs/Cargo.toml`:
```toml
version = "1.1.4"
```
to:
```toml
version = "1.1.5"
```

- [ ] **Step 2: Final verification**

Run (in `sub-rs/`):
```bash
cargo test
cargo build --release
```
Expected: all tests pass, release build finishes.

- [ ] **Step 3: Commit**

```bash
git add sub-rs/Cargo.toml sub-rs/Cargo.lock
git commit -m "Bump version to 1.1.5"
```

- [ ] **Step 4: Tag and push** (creates the release via CI — Windows/Linux/macOS + zip/tar.gz)

```bash
git tag v1.1.5
git push origin master
git push origin v1.1.5
```
Expected: GitHub Actions `Release` workflow run #N builds all four platform
targets and attaches the archives to a `v1.1.5` release.

- [ ] **Step 5: Confirm release**

Check https://github.com/saeedrss/subsourceCLI/actions and the Releases page.
Expected: 4 binaries (x86_64-windows.zip, x86_64-linux.tar.gz,
x86_64-macos.tar.gz, aarch64-macos.tar.gz) attached to v1.1.5.