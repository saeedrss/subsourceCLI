# Changelog

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
