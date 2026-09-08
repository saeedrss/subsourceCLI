# Connection Methods for sub-rs

Date: 2026-09-08
Status: Approved by user

## Summary

Add four selectable connection methods for reaching the SubSource API, to work
around ISP-level blocking/rate-limiting of `api.subsource.net`:

1. **Direct** — no proxy, straight to the API.
2. **System proxy** — automatic detection (reqwest default behavior).
3. **Manual proxy** — user-supplied URL in a textbox.
4. **Cloudflare** — route through a Cloudflare Worker that transparently
   forwards to `api.subsource.net` (bypasses ISP blocking of that host).

The Worker is the first thing to try: write/deploy it, prove it works, and only
then wire all four modes into the app and ship.

## Motivation

- User's ISP blocks or throttles `api.subsource.net` directly.
- A `*.workers.dev` URL is reachable, so the Worker acts as a transparent
  reverse proxy.
- Manual proxy support already exists (`--proxy`, checkbox+textbox in GUI);
  this feature formalizes the full set of options and adds Cloudflare.

## Architecture

### New module `src/connect.rs`

Shared connection type used by the whole app:

```rust
pub enum Mode { Direct, System, Manual, Cloudflare }
// serde lowercase + clap ValueEnum: "direct" | "system" | "manual" | "cloudflare"

pub struct Connection {
    pub mode: Mode,
    pub manual_proxy: Option<String>, // http://ip:port or socks5://ip:port
    pub worker_url: Option<String>,   // https://<name>.workers.dev
}
```

`connect.rs` also owns a single `build_reqwest(&Connection) -> Result<reqwest::blocking::Client>`:

| Mode | Builder config | API base |
|---|---|---|
| Direct | `.no_proxy()` | `https://api.subsource.net/api/v1` |
| System | default builder (system-proxy feature auto-detects) | same |
| Manual | `.proxy(reqwest::Proxy::all(url)?)` | same |
| Cloudflare | `.no_proxy()` | `https://<worker>/api/v1` |

Notes:
- reqwest 0.12 enables `system-proxy` by default, so "System" is the current
  default behavior; "Direct" must call `.no_proxy()` explicitly.
- Cloudflare mode uses `.no_proxy()` too — the Worker is reached directly and
  does its own forwarding.
- `client.rs`, `ads.rs`, `updater.rs` change signature from
  `proxy: Option<String>` / `Option<&str>` to `conn: &Connection`.

### Cloudflare Worker (`cloudflare-worker/`)

- `worker.js` — transparent reverse proxy:
  - `new URL(request.url)` → target = `https://api.subsource.net` + pathname + search.
  - Copy headers, delete `Host`.
  - Stream body for non-GET requests.
  - `X-API-Key` and all other headers pass through untouched.
- `wrangler.toml` — `name = "subsource-proxy"`, `main = "worker.js"`.
- Client request paths unchanged: only `API_BASE` becomes worker origin + `/api/v1`.

## Config file

`~/.config/subsource/config.json` extends the current `{api_key, proxy}` shape:

```json
{
  "api_key": "...",
  "connect": "manual",
  "proxy": "http://ip:port",
  "worker": "https://name.workers.dev"
}
```

- `connect` absent but `proxy` present → treated as `manual` (backward compat).
- `connect` absent and no proxy → `direct`.

## CLI

- `--connect <direct|system|manual|cloudflare>` (default `direct`)
- `--proxy <url>` — manual proxy URL (existing flag, keeps meaning)
- `--worker <url>` — Cloudflare Worker URL (new)
- Explicit `--connect` with a missing required value (e.g. `manual` with no
  `--proxy`, or `cloudflare` with no `--worker`) is an error at client build.
- `--proxy` alone (no `--connect`) still implies manual mode for backward compat.

## GUI

- Replace the single Proxy checkbox + textbox with radio buttons:
  **Direct / System proxy / Manual proxy / Cloudflare worker**.
- Radio selection drives a textbox:
  - Manual → proxy URL textbox enabled
  - Cloudflare → worker-URL textbox enabled
  - Direct / System → no textbox
- Mode + values persisted to config on scan start (mirrors current
  `save_config` behavior).
- Ads loader and update checker reuse the same `Connection`.

## Test-first sequence (per user request)

1. Implement `cloudflare-worker/` + add `connect.rs` and at least the
   Cloudflare path wired into the CLI so it is reachable end-to-end.
2. User deploys the worker (`wrangler deploy` or Cloudflare dashboard paste).
3. Prove it works: run a search + a subtitle download through the Worker URL.
4. Only if the Cloudflare route succeeds: finish the GUI radio UI, docs
   (README fa/en, CHANGELOG), bump version, `cargo build --release`, push.

## Out of scope

- Hiding the API key server-side in the Worker (user chose bypass only).
- Per-endpoint proxy rules or NO_PROXY customization.
- Worker authentication / rate limiting / caching.
- Converting blocking reqwest to async.