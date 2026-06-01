# subsourceCLI

Downloads Farsi/Persian subtitles for video files via the
[SubSource](https://subsource.net) API. Extracts the best match as
`video.fa.srt` alongside the video and keeps top-N ZIP backups.

## Requirements

- Python 3.8+
- `requests` (see `requirements.txt`)

## Install

```bash
pip install -r requirements.txt
pip install .
```

Or in editable mode:

```bash
pip install -e .
```

## Usage

```bash
subsourceCLI --directory "G:\1080\MyShow" --top 5
```

Or via `python -m`:

```bash
python -m subsourceCLI --directory "G:\1080\MyShow"
```

### Options

| Argument       | Default                    | Description                          |
|----------------|----------------------------|--------------------------------------|
| `--directory`  | `.`                        | Directory to scan for video files    |
| `--top`        | `5`                        | Number of subtitle candidates to keep|
| `--api-key`    | `SUBSOURCE_API_KEY` env var | API key (**required**)              |
| `--dry-run`    | —                          | Log actions without downloading      |
| `--no-recursive` | —                        | Only scan directory root             |
| `--proxy`      | `None`                     | Proxy URL (e.g. `http://127.0.0.1:8080`) |

> **API key is required.** Provide via `--api-key` or set the `SUBSOURCE_API_KEY`
> environment variable. There is no hardcoded fallback.

### Output layout

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
