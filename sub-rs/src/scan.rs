#![allow(clippy::comparison_chain)]

use crate::client::{Client, SubtitleEntry};
use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use zip::ZipArchive;

pub const VIDEO_EXTENSIONS: &[&str] = &[".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".m4v", ".webm"];
pub const SUBTITLE_EXTENSIONS: &[&str] = &[".srt", ".ass", ".ssa", ".sub", ".vtt", ".txt"];

static BRACKETS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\[\(].*?[\]\)]").unwrap());
static BRACES_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{.*?\}").unwrap());
static EPISODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[Ss](\d{1,2})[Ee](\d{1,2})").unwrap());
static YEAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(19\d{2}|20[0-3]\d)").unwrap()); // ; pony: no lookaround in regex crate
static TECH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\d{3,4}p|BluRay|BRRip|HDRip|WEB-DL|WEBRip|DVDRip|HDTV|AMZN|NF|\
         x264|x265|HEVC|H\.264|AVC|h264|h265|\
         DDP\d\.\d|AC3|AAC\d\.\d|DTS|Atmos|DDPA\d\.\d|\
         ETHEL|EDITH|EZTV|TGx|rartv|YIFY|NAISU|SHORTBREHD|iCEBERG|AGLET|TBD|t3nzin|\
         VideoGod|WORLD|CONDITION|SyncUp|GainfulCapedHyraxOfPiety|SuccessfulCrab|EniaHD|\
         APEX|SPARKS|GECKOS|DRACULA|ROVERS|LAZY|DEFLATE|DEMAND|NTb|KiNGS|Cinefeel|\
         TRASHCAN|GalaxyTV|FLUX|HONE|KOGi|mSD|BAMBOOLEZ|MiNX|ION10|PSA|RARBG|YTS|AMIABLE|\
         REMUX|Complete|UNRATED|UNCUT|PROPER|REPACK|EXTENDED|DIRECTORS?\.?CUT|DC|\
         Sample|Samples|Trailer|Featurette|\
         S\d{1,2}E\d{1,2}|Season\s*\d+|Episode\s*\d+|\
         www\..*?\.org|www\..*?\.com|www\..*?\.net",
    )
    .unwrap()
});

#[derive(Debug, Default)]
pub struct FileInfo {
    pub title: String,
    pub year: Option<String>,
    pub season: Option<String>,
    pub episode: Option<String>,
    pub is_episode: bool,
    pub search_query: String,
}

pub struct Stats {
    pub scanned: u32,
    pub found: u32,
    pub downloaded: u32,
    pub errors: u32,
    pub skipped: u32,
}

impl Stats {
    pub fn new() -> Self {
        Stats { scanned: 0, found: 0, downloaded: 0, errors: 0, skipped: 0 }
    }
}

pub fn collect_videos(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut videos = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    let mut i = 0;
    while i < dirs.len() {
        if let Ok(entries) = std::fs::read_dir(&dirs[i]) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && recursive {
                    dirs.push(path);
                } else if path.is_file() {
                    let name = path.file_name().unwrap().to_string_lossy().to_lowercase();
                    if !name.contains("sample")
                        && VIDEO_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
                    {
                        videos.push(path);
                    }
                }
            }
        }
        i += 1;
    }
    videos.sort();
    videos
}

pub fn parse_filename(filename: &str) -> FileInfo {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let mut name = stem.to_string();
    let no_brackets = BRACKETS_RE.replace_all(&name, " ");
    let mut info = FileInfo::default();

    if let Some(caps) = EPISODE_RE.captures(&name) {
        info.season = Some(caps[1].to_string());
        info.episode = Some(caps[2].to_string());
        info.is_episode = true;
    }

    if let Some(m) = YEAR_RE.find(&no_brackets) {
        info.year = Some(m.as_str().to_string());
    }

    if info.is_episode {
        if let Some(m) = EPISODE_RE.find(&name) {
            name.truncate(m.end());
        }
    }

    let mut clean = BRACKETS_RE.replace_all(&name, " ").to_string();
    clean = BRACES_RE.replace_all(&clean, " ").to_string();
    clean = TECH_RE.replace_all(&clean, " ").to_string();
    clean = clean.replace(['.', '_'], " ");
    clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = clean.trim_end_matches('-').trim().to_string();
    info.title = clean.clone();

    let search = if let Some(m) = EPISODE_RE.find(&clean) {
        clean[..m.start()].trim().to_string()
    } else {
        clean
    };
    info.search_query = if search.is_empty() { info.title.clone() } else { search };
    // ; pony: strip year from title and search query (passed separately to API)
    if let Some(ref year) = info.year {
        info.title = info.title.replace(year, "").split_whitespace().collect::<Vec<_>>().join(" ");
        info.search_query = info.search_query.replace(year, "").split_whitespace().collect::<Vec<_>>().join(" ");
    }

    info
}

fn match_best_movie<'a>(
    results: &'a [crate::client::Movie],
    file_info: &FileInfo,
) -> Option<&'a crate::client::Movie> {
    let mut best = None;
    let mut best_score = 0.0;

    for movie in results {
        for title in [movie.title.as_deref(), movie.alternate_title.as_deref()]
            .into_iter()
            .flatten()
        {
            let mut score =
                strsim::jaro_winkler(&title.to_lowercase(), &file_info.title.to_lowercase());

            if file_info.is_episode {
                if movie.media_type.as_deref().is_some_and(|t| t == "tvseries" || t == "series") {
                    score += 0.15;
                }
                if let Some(ref s) = file_info.season {
                    let target: i32 = s.parse().unwrap_or(0);
                    if let Some(ms) = movie.season {
                        score += if ms == target { 0.3 } else { -0.5 };
                    }
                }
            }

            if score > best_score && score > 0.6 {
                best_score = score;
                best = Some(movie);
            }
        }
    }
    best
}

fn extract_season_from_release_info(release: &str) -> Option<i32> {
    let patterns = [
        Regex::new(r"(?i)Season\s*(\d{1,2})").unwrap(),
        Regex::new(r"(?i)Season(\d{1,2})").unwrap(),
        Regex::new(r"(?i)\bS(\d{1,2})\b").unwrap(),
        Regex::new(r"(?i)S(\d{1,2})E\d{1,2}").unwrap(),
    ];
    for p in &patterns {
        if let Some(c) = p.captures(release) {
            return c[1].parse().ok();
        }
    }
    None
}

fn filter_by_season<'a>(
    subs: &'a [SubtitleEntry],
    target: &str,
) -> Vec<&'a SubtitleEntry> {
    let target: i32 = match target.parse() {
        Ok(t) => t,
        Err(_) => return subs.iter().collect(),
    };
    subs.iter()
        .filter(|s| {
            let rel = s
                .release_info
                .as_deref()
                .map(|v| v.join(" "))
                .unwrap_or_default();
            match extract_season_from_release_info(&rel) {
                Some(season) => season == target,
                None => true,
            }
        })
        .collect()
}

fn score_subtitle(sub: &SubtitleEntry) -> (f64, i32) {
    let r = sub.rating.as_ref();
    let good = r.map(|r| r.good).unwrap_or(0);
    let total = r.map(|r| r.total).unwrap_or(1).max(1);
    (good as f64 / total as f64, sub.downloads.unwrap_or(0))
}

pub fn process_video(
    video_path: &Path,
    client: &Client,
    top_n: usize,
    dry_run: bool,
    log: &dyn Fn(&str),
) -> Result<bool> {
    log(&format!(
        "\n[FILE] Processing: {}\n",
        video_path.file_name().unwrap_or_default().to_string_lossy()
    ));

    let file_info =
        parse_filename(&video_path.file_name().unwrap_or_default().to_string_lossy());
    log(&format!(
        "  [PARSE] Title='{}', S={:?}, E={:?}\n",
        file_info.title,
        file_info.season.as_deref().unwrap_or(""),
        file_info.episode.as_deref().unwrap_or("")
    ));

    let base_name = video_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let parent_dir = video_path.parent().unwrap();
    let sub_dir = parent_dir.join("sub");
    std::fs::create_dir_all(&sub_dir)?;

    let best_path = parent_dir.join(format!("{}.fa.srt", base_name));
    if best_path.exists() {
        log(&format!(
            "  [INFO] Best subtitle already extracted: {}\n",
            best_path.file_name().unwrap().to_string_lossy()
        ));
        let existing = std::fs::read_dir(&sub_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{}_sub", base_name))
            })
            .count();
        if existing >= top_n {
            log("  [INFO] Backup ZIPs also present\n");
            return Ok(true);
        }
    }

    let query = if file_info.search_query.is_empty() {
        let folder = parent_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        parse_filename(folder).search_query
    } else {
        file_info.search_query.clone()
    };

    log(&format!("  [SEARCH] Searching: '{}'\n", query));
    let results = client.search_movie(&query, file_info.year.as_deref())?;

    if results.is_empty() {
        log("  [ERROR] No search results\n");
        return Ok(false);
    }
    log(&format!("  [COUNT] API returned {} result(s)\n", results.len()));

    let best = match_best_movie(&results, &file_info)
        .ok_or_else(|| anyhow::anyhow!("No good match found"))?;
    let movie_id_str = best.movie_id.map(|id| id.to_string()).unwrap_or_default();
    log(&format!(
        "  [SUCCESS] Found: '{}' [ID: {}]\n",
        best.title.as_deref().unwrap_or("?"),
        movie_id_str
    ));

    log("  [LANG] Searching for Farsi/Persian subtitles...\n");
    let lang_codes = ["farsi_persian", "farsi", "persian", "fa"];
    let subtitles = lang_codes.iter().find_map(|lang| {
        log(&format!("    [TRY] Language code: '{}'...\n", lang));
        client.get_subtitles(&movie_id_str, lang).ok().filter(|s| !s.is_empty())
    });

    let subtitles = match subtitles {
        Some(s) => s,
        None => {
            log("  [ERROR] No Farsi/Persian subtitles found\n");
            return Ok(false);
        }
    };

    log(&format!(
        "  [FILTER] Filtering for Season {:?}...\n",
        file_info.season
    ));
    let season_filtered =
        filter_by_season(&subtitles, file_info.season.as_deref().unwrap_or(""));
    let pool: Vec<&SubtitleEntry> = if season_filtered.is_empty() {
        log("  [WARNING] No season match, using all available...\n");
        subtitles.iter().collect()
    } else {
        season_filtered
    };

    let filtered: Vec<&SubtitleEntry> = if file_info.is_episode {
        let s = file_info.season.as_deref().unwrap_or("");
        let e = file_info.episode.as_deref().unwrap_or("");
        let ep_matches: Vec<&SubtitleEntry> = pool
            .iter()
            .copied()
            .filter(|sub| {
                let rel = sub
                    .release_info
                    .as_deref()
                    .map(|v| v.join(" "))
                    .unwrap_or_default();
                let com = sub.commentary.as_deref().unwrap_or("");
                let combined = format!("{} {}", rel, com);
                [
                    format!("S{}E{}", s, e),
                    format!("s{}e{}", s, e),
                    format!(
                        "{}x{}",
                        s.parse::<i32>().unwrap_or(0),
                        e.parse::<i32>().unwrap_or(0)
                    ),
                ]
                .iter()
                .any(|p| combined.contains(p.as_str()))
            })
            .collect();
        if !ep_matches.is_empty() { ep_matches } else { pool }
    } else {
        pool
    };

    if filtered.is_empty() {
        log("  [ERROR] No matching subtitles after filtering\n");
        return Ok(false);
    }

    let mut sorted: Vec<&&SubtitleEntry> = filtered.iter().collect();
    sorted.sort_by(|a, b| {
        score_subtitle(b)
            .partial_cmp(&score_subtitle(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top_subs: Vec<&&SubtitleEntry> = sorted.into_iter().take(top_n).collect();

    let mut extracted = false;
    for (i, sub) in top_subs.iter().enumerate() {
        let sub_id_str = match sub.subtitle_id {
            Some(id) => id.to_string(),
            None => continue,
        };
        let release = sub.release_info.as_deref().map(|v| v.join(" ")).unwrap_or_default();
        let safe_relay: String = release
            .chars()
            .take(50)
            .collect::<String>()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let zip_name = format!("{}_sub{}_{}.zip", base_name, i + 1, safe_relay);
        let zip_path = sub_dir.join(&zip_name);
        let is_best = i == 0;
        let action = if is_best { "EXTRACT" } else { "BACKUP" };

        let r = sub.rating.as_ref();
        let rating_str = format!(
            "{}/{}",
            r.map(|r| r.good).unwrap_or(0),
            r.map(|r| r.total).unwrap_or(0)
        );
        log(&format!(
            "\n    [{}/{}] [{}] {}...\n         Rating: {}, Downloads: {}\n",
            i + 1,
            top_n,
            action,
            release.chars().take(50).collect::<String>(),
            rating_str,
            sub.downloads.unwrap_or(0)
        ));

        if dry_run {
            log(&format!("         [DRYRUN] Would save: sub\\{}\n", zip_name));
            extracted = true;
            continue;
        }

        if zip_path.exists() {
            log(&format!("         [INFO] Already exists: {}\n", zip_name));
            if is_best && !best_path.exists() {
                log("         [EXTRACT] Extracting existing best match...\n");
                if extract_and_rename_best(&zip_path, video_path, &file_info, log).unwrap_or(false) {
                    extracted = true;
                }
            }
            continue;
        }

        log("         [DOWNLOAD] Downloading...\n");
        match client.download_zip(&sub_id_str, &zip_path) {
            Ok(()) => {
                log(&format!("         [SUCCESS] Saved: {}\n", zip_name));
                if is_best {
                    log("         [EXTRACT] Extracting best match...\n");
                    if extract_and_rename_best(&zip_path, video_path, &file_info, log)
                        .unwrap_or(false)
                    {
                        extracted = true;
                    }
                }
            }
            Err(e) => log(&format!("         [ERROR] Download failed: {}\n", e)),
        }
    }

    if extracted || dry_run {
        log("\n  [SUCCESS] Processed\n");
        Ok(true)
    } else {
        log("\n  [ERROR] Failed to download any subtitles\n");
        Ok(false)
    }
}

pub fn extract_and_rename_best(
    zip_path: &Path,
    video_path: &Path,
    file_info: &FileInfo,
    log: &dyn Fn(&str),
) -> Result<bool> {
    let stem = video_path.file_stem().unwrap_or_default().to_string_lossy();
    let extract_dir = video_path.parent().unwrap().join(format!("_temp_subs_{}", stem));
    let parent = video_path.parent().unwrap();

    let file = std::fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    let sub_indices: Vec<usize> = (0..archive.len())
        .filter(|i| {
            archive.by_index(*i).ok().map_or(false, |f| {
                let name = f.name().to_lowercase();
                SUBTITLE_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
            })
        })
        .collect();

    if sub_indices.is_empty() {
        log("    [ERROR] No subtitle files found in ZIP\n");
        return Ok(false);
    }
    log(&format!("    [ZIP] ZIP contains {} subtitle file(s)\n", sub_indices.len()));

    std::fs::create_dir_all(&extract_dir)?;
    for &i in &sub_indices {
        let mut file = archive.by_index(i)?;
        let outpath = extract_dir.join(file.name());
        if let Some(p) = outpath.parent() {
            std::fs::create_dir_all(p)?;
        }
        if !file.is_dir() {
            let mut out = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut out)?;
        }
    }

    let result = if sub_indices.len() == 1 {
        let name = archive.by_index(sub_indices[0]).unwrap().name().to_string();
        let src = extract_dir.join(&name);
        let suffix = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let dst = parent.join(format!("{}.fa{}", stem, suffix));
        if dst.exists() {
            std::fs::rename(&dst, dst.with_extension("srt.backup"))?;
        }
        std::fs::rename(&src, &dst)?;
        log(&format!(
            "    [OK] Extracted and renamed to: {}\n",
            dst.file_name().unwrap().to_string_lossy()
        ));
        true
    } else if let Some(ref ep) = file_info.episode {
        let target_ep = ep.parse::<u32>().unwrap_or(0);
        let found = sub_indices.iter().find_map(|&i| {
            let file = archive.by_index(i).ok()?;
            let fname = Path::new(file.name()).file_name()?.to_str()?;
            let pats: [Regex; 3] = [
                Regex::new(&format!(r"(?i)[eE][pP]?{}", target_ep)).unwrap(),
                Regex::new(&format!(r"(?i)episode[\s\.]?{:02}", target_ep)).unwrap(),
                Regex::new(&format!(r"\b{:02}\b", target_ep)).unwrap(),
            ];
            if pats.iter().any(|p| p.is_match(fname)) {
                Some(file.name().to_string())
            } else {
                None
            }
        });
        match found {
            Some(f) => {
                let src = extract_dir.join(&f);
                let suffix = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                let dst = parent.join(format!("{}.fa{}", stem, suffix));
                if dst.exists() {
                    std::fs::rename(&dst, dst.with_extension("srt.backup"))?;
                }
                std::fs::rename(&src, &dst)?;
                log(&format!(
                    "    [OK] Matched episode {}: {}\n",
                    target_ep,
                    dst.file_name().unwrap().to_string_lossy()
                ));
                true
            }
            None => {
                log(&format!(
                    "    [WARNING] Could not match episode {} in multi-file ZIP\n",
                    target_ep
                ));
                for (j, &i) in sub_indices.iter().enumerate().take(5) {
                    if let Ok(f) = archive.by_index(i) {
                        log(&format!(
                            "       {}. {}\n",
                            j + 1,
                            Path::new(f.name())
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                        ));
                    }
                }
                false
            }
        }
    } else {
        let name = archive.by_index(sub_indices[0]).unwrap().name().to_string();
        let src = extract_dir.join(&name);
        let suffix = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let dst = parent.join(format!("{}.fa{}", stem, suffix));
        if dst.exists() {
            std::fs::rename(&dst, dst.with_extension("srt.backup"))?;
        }
        std::fs::rename(&src, &dst)?;
        log(&format!(
            "    [OK] Extracted first file: {}\n",
            dst.file_name().unwrap().to_string_lossy()
        ));
        true
    };

    let _ = std::fs::remove_dir_all(&extract_dir);
    Ok(result)
}

pub fn scan_directory(
    dir: &Path,
    client: &Client,
    top_n: usize,
    recursive: bool,
    dry_run: bool,
    log: &dyn Fn(&str),
) -> Result<Stats> {
    let mut stats = Stats::new();

    log(&format!(
        "\n{}\n[SEARCH] Scanning: {}\n   Mode: Extract best .fa.srt + keep top {} ZIPs in sub\\\n{}\n",
        "=".repeat(70),
        dir.display(),
        top_n,
        "=".repeat(70)
    ));

    let videos = collect_videos(dir, recursive);
    log(&format!("Found {} video file(s)\n", videos.len()));

    for video in &videos {
        match process_video(video, client, top_n, dry_run, log) {
            Ok(true) => {
                stats.found += 1;
                stats.downloaded += 1;
            }
            Ok(false) => stats.errors += 1,
            Err(e) => {
                log(&format!("  [ERROR] {}\n", e));
                stats.errors += 1;
            }
        }
        stats.scanned += 1;
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    Ok(stats)
}
