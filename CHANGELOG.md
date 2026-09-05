# Changelog

## [1.1.4] - 2026-09-05

### Added
- Farsi ad & brand cleanup: deletes whole subtitle lines whose text matches
  known ad frames (digimoviez, دیجی موویز, EBTV, ...) or brand frames
  (امپایربست / empire best tv). Enabled via `--clean-ads` (CLI) or the
  "Remove Farsi ads" checkbox (GUI). Applies to `fa` subtitles only
- GUI: Persian font (Vazirmatn) embedded so Farsi text renders instead of
  empty boxes
- Built-in API key fallback used when `--api-key`, the `SUBSOURCE_API_KEY`
  env var, and the config file are all absent

### Changed
- Partial success: a video is considered done when at least one subtitle was
  downloaded, extracted from a saved backup, or already present — the GUI now
  shows `done X/Y` and stats count actual downloads

### Fixed
- When the best match download fails, the best available saved backup ZIP is
  now extracted as a fallback instead of failing the whole file

## [1.1.3] - 2026-08-08

### Added
- `--no-lang-suffix` flag (CLI) and "No Lang Suffix" checkbox (GUI): save the
  best match as `movie.srt` instead of `movie.fa.srt` for any language

### Fixed
- `--skip-existing` now skips a video when either `movie.srt` or
  `movie.{lang}.srt` already exists, regardless of suffix mode

## [1.1.2] - 2026-07-29

### Added
- Version update checker: checks GitHub for new releases at startup (CLI + GUI)
- GUI: "Update Available" window with release notes when a newer version exists
- CLI: prints update notification with changelog after scan completes
- `Global Log` entry in left panel: click to return to global log after viewing a file's detail log
- Real-time per-file log streaming in the detail panel (updates live during scan)

### Fixed
- Title parsing: "Extended Cut" now removed as a complete phrase from filenames
- GUI: console window hidden on Windows when running in `--gui` mode

## [1.1.1] - 2026-07-28

### Fixed
- Filename parsing truncation for movies to exclude tech specs from search query

## [1.1.0] - 2026-07-28

### Added
- Multi-language support with `--lang` flag and 19 languages
- GUI language dropdown selector
- Proxy toggle checkbox in GUI
- Rust GUI (egui) desktop application with `--gui` flag
- Persian/Farsi language support in GUI
