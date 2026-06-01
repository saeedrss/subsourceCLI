# SubSource Farsi Subtitle Downloader

Downloads Farsi/Persian subtitles for video files via the
[SubSource](https://subsource.net) API. Extracts the best match as
`video.fa.srt` alongside the video and keeps top-N ZIP backups.

Three interfaces are available:

- **GUI** (`gui.py`) — DearPyGui desktop application with bilingual EN/FA support
- **Package** (`subsourceCLI/`) — pip-installable CLI with no hardcoded fallback
- **Script** (`script.py`) — standalone single-file CLI with built-in fallback key

## Quick Start (GUI)

```bash
pip install dearpygui arabic-reshaper python-bidi requests
python gui.py
```

## Quick Start (CLI)

```bash
pip install -r requirements.txt
python script.py --directory "G:\1080\MyShow" --top 5
```

Or install the package:

```bash
pip install -e subsourceCLI
subsourceCLI --directory "G:\1080\MyShow"
```

## Requirements

- Python 3.8+
- `requests` (all interfaces)
- `dearpygui`, `arabic-reshaper`, `python-bidi` (GUI only)

## Usage

### GUI

```bash
python gui.py
```

- Select a directory, configure your API key in **Settings**, click **Start Scan**
- Toggle language EN/FA in the settings bar
- First run prompts language selection

### CLI (script.py)

```bash
python script.py --directory "G:\1080\MyShow" --top 5
```

| Argument | Default | Description |
|---|---|---|
| `--directory` | `.` | Directory to scan for video files |
| `--top` | `5` | Number of subtitle candidates to keep |
| `--api-key` | `SUBSOURCE_API_KEY` env var | API key (falls back to hardcoded key) |
| `--dry-run` | — | Log actions without downloading |
| `--no-recursive` | — | Only scan directory root |
| `--proxy` | `None` | Proxy URL (e.g. `http://127.0.0.1:8080`) |

### Package (subsourceCLI)

```bash
subsourceCLI --directory "G:\1080\MyShow" --top 5
```

> **API key is required.** Provide via `--api-key` or set the `SUBSOURCE_API_KEY`
> environment variable. The package has no hardcoded fallback.

## Output layout

```
video.mkv
video.fa.srt          ← best match (extracted and renamed)
sub/
  video_sub1_*.zip    ← best match (ZIP backup)
  video_sub2_*.zip    ← alternatives (up to --top)
```

## Features

- Parses `S01E01` style episode markers, matches subtitles by season & episode
- Falls back to folder name when filename has no recognizable title
- Applies 1-second rate limiting between API calls
- Sanitizes non-ASCII characters for Windows console compatibility
- GUI: bilingual English/Farsi with persistent language preference
- GUI: real-time per-file progress and log output

## Repository

- GitHub: https://github.com/saeedrss/subsourceCLI
- Releases: https://github.com/saeedrss/subsourceCLI/releases
- Author: [saeedrss](https://github.com/saeedrss)
