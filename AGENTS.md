# AGENTS.md — sub

Single-file Python CLI that downloads Farsi subtitles for video files via the
SubSource API, picks the best match, extracts `.fa.srt` alongside the video, and
keeps top-N ZIP backups.

## Quick start

```bash
pip install requests
python script.py --directory "G:\1080\MyShow" --top 5
```

No test runner, formatter, linter, or CI is configured.

## Usage

| Argument | Default | Purpose |
|---|---|---|
| `--directory` | `.` | Scan path |
| `--top` | `5` | Number of subtitle candidates to keep |
| `--api-key` | env var `SUBSOURCE_API_KEY` then hardcoded fallback | Authentication |
| `--dry-run` | — | Log what would happen without downloading |
| `--no-recursive` | — | Only scan directory root |
| `--proxy` | `None` | Proxy URL (e.g. `http://127.0.0.1:8080`) |

Run `python script.py --help` for all options.

## Important notes

- **API key** is hardcoded at `script.py:15` as a fallback. Prefer
  `SUBSOURCE_API_KEY` env var or `--api-key` to avoid committing the key.
- **External dependency**: `requests` must be installed (`pip install requests`).
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
  into the `SubSourceAPI` class.
