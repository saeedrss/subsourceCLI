<p align="center">
  <img src="icon.svg" width="160" alt="SubSource logo"/>
</p>

# SubSource Subtitle Downloader

<div align="center">

[📖 فارسی — مستندات فارسی (faREADME.md)](faREADME.md)

Downloads subtitles for your movies and TV shows via the
[SubSource](https://subsource.net) API — best match saved next to the video,
top-N ZIP backups kept in `sub/`. Supports **19 languages**.

</div>

## 🖥️ Rust GUI (primary)

A single, cross-platform desktop app (Windows / macOS / Linux) built with egui —
no Python or runtime needed. Just download the binary and run it:

```bash
./sub-rs --gui
```

### GUI features

- 📁 **Select directory** — pick a folder, scan recursively into sub-folders
- 🌐 **19 subtitle languages** — dropdown selector (Farsi, English, Arabic, ...)
- 🚫 **Skip Existing** — skip videos that already have a subtitle
- 🏷️ **No Lang Suffix** — save as `movie.srt` instead of `movie.fa.srt`
- 🔀 **Language toggle** — switch the interface between **English / فارسی**
- 🔧 **Proxy** — enable/disable checkbox with live reload
- 🧪 **Dry Run** — test without downloading anything
- 📋 **Live logs** — per-file detail log + global log streamed in real time
- 🔔 **Update checker** — notifies when a new release is available

## ⚡ Quick Start

Pre-built binaries on the [Releases page](https://github.com/saeedrss/subsourceCLI/releases):

```bash
# GUI — launch the desktop app
./sub-rs --gui

# CLI — scan a directory and download best subtitles (default Farsi)
./sub-rs --directory "/path/to/videos"
```

Or build from source:

```bash
cd sub-rs
cargo build --release
./target/release/sub-rs --gui
```

## 🧰 CLI options

| Argument | Default | Description |
|---|---|---|
| `-d, --directory` | `.` | Directory to scan for video files |
| `--top` | `5` | Number of subtitle candidates to keep |
| `-l, --lang` | `fa` | Subtitle language code (`fa`, `en`, `ar`, ...) |
| `--api-key` | `SUBSOURCE_API_KEY` env or config file | API key (no hardcoded fallback) |
| `--dry-run` | — | Log actions without downloading |
| `--no-recursive` | — | Only scan directory root |
| `--proxy` | `None` | Proxy URL (e.g. `http://127.0.0.1:8080`) |
| `--skip-existing` | — | Skip videos that already have a subtitle |
| `--no-lang-suffix` | — | Save as `movie.srt` without the language suffix |
| `--gui` | — | Launch GUI instead of CLI |

API key resolution: `--api-key` > `SUBSOURCE_API_KEY` env var > `~/.config/subsource/config.json`.

## 📁 Output layout

```
video.mkv
video.fa.srt          ← best match (extracted and renamed; or video.srt with --no-lang-suffix)
sub/
  video_sub1_*.zip    ← best match (ZIP backup)
  video_sub2_*.zip    ← alternatives (up to --top)
```

## ✨ Features

- Single ~10 MB Rust binary — CLI + GUI in one, no Python/runtime needed
- Cross-platform egui GUI: Windows, macOS, Linux
- Parses `S01E01` episode markers, matches subtitles by season & episode
- Movie names cleaned of tech specs (codec, resolution, groups) for better matches
- Falls back to folder name when the filename has no recognizable title
- 19 subtitle languages, English/Farsi interface
- Skip-existing detects both `movie.srt` and `movie.{lang}.srt`
- 1-second rate limiting between API calls
- Update checker with release notes

## 🐍 Legacy (Python)

Older Python interfaces are kept for reference but are **no longer the primary
way** to use the app:

```bash
python gui.py                                   # DearPyGui desktop app
python script.py --directory "G:\1080\MyShow"   # CLI
pip install -e subsourceCLI && subsourceCLI     # installable package
```

## 📚 Repository

- GitHub: https://github.com/saeedrss/subsourceCLI
- Releases: https://github.com/saeedrss/subsourceCLI/releases
- Author: [saeedrss](https://github.com/saeedrss)
