# AGENTS.md — sub

Multi-interface Farsi subtitle downloader. **Rust** (`sub-rs/`) is the primary
version (CLI + egui GUI in a single binary). Python versions (`script.py`,
`gui.py`, `subsourceCLI/`) are legacy.

## Rust binary (`sub-rs/`)

Single-file Python CLI that downloads Farsi subtitles for video files via the
SubSource API, picks the best match, extracts `.fa.srt` alongside the video, and
keeps top-N ZIP backups.

```bash
cd sub-rs
cargo build --release
./target/release/sub-rs --directory "G:\1080\MyShow" --top 5
```

No test runner, formatter, linter, or CI is configured outside of GitHub Actions.

## Usage

| Argument | Default | Purpose |
|---|---|---|
| `--directory` | `.` | Scan path |
| `--top` | `5` | Number of subtitle candidates to keep |
| `--api-key` | env var `SUBSOURCE_API_KEY` then `~/.config/subsource/config.json` | Authentication |
| `--dry-run` | — | Log what would happen without downloading |
| `--no-recursive` | — | Only scan directory root |
| `--proxy` | `None` | Proxy URL (e.g. `http://127.0.0.1:8080`) |
| `--gui` | — | Launch egui desktop GUI |

Run `./target/release/sub-rs --help` for all options.

## Important notes

- **API key** resolution: `--api-key` > `SUBSOURCE_API_KEY` env var > config file.
  No hardcoded fallback. Config saved to `~/.config/subsource/config.json`.
- **External dependency**: Only Rust toolchain needed (`cargo build --release`).
- **Output layout** in the target directory:
  ```
  video.mkv
  video.fa.srt          ← best match (extracted and renamed)
  sub/
    video_sub1_*.zip    ← best match (ZIP backup)
    video_sub2_*.zip    ← alternatives (up to `--top`)
  ```
- **Episode matching**: supports `S01E01` style parsing, season filtering, and
  episode-level selection from multi-file ZIPs.
- **Rate limiting**: 1-second delay (`REQUEST_DELAY`) between API calls, built
  into the `SubSourceAPI` class in `client.rs`.

## Modules

| File | Purpose |
|---|---|
| `src/main.rs` | CLI arg parsing (clap), config load/save, dispatch to CLI or GUI |
| `src/client.rs` | SubSource API client (search, subtitles, download) |
| `src/scan.rs` | File scanning, filename parsing, movie matching, ZIP extraction |
| `src/gui.rs` | egui desktop app (file list, log, settings, background thread) |
