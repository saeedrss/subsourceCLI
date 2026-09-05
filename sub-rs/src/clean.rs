use std::path::Path;
use std::sync::LazyLock;

pub const AD_FINGERPRINTS: &[&str] = &[
    "digimoviez",
    "digimovie",
    "@digimoviez",
    "EBTV",
    "دیجی موویز",
    "دیجیمویز",
    "جهت اطلاع از جدیدترین آدرس سایت",
    "دیجی موویز را در شبکه های اجتماعی دنبال کنید",
];

pub const BRAND_FINGERPRINTS: &[&str] = &[
    "امپایربست",
    "اِمپایر بست تی وی",
    "امپایر بست تی وی",
    "empire best tv",
];

static AD_FLAT: LazyLock<Vec<String>> = LazyLock::new(|| {
    AD_FINGERPRINTS.iter().map(|f| flatten(f).to_lowercase()).collect()
});
static BRAND_FLAT: LazyLock<Vec<String>> = LazyLock::new(|| {
    BRAND_FINGERPRINTS.iter().map(|f| flatten(f).to_lowercase()).collect()
});

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CleanCounts {
    pub ads_removed: u32,
    pub brand_lines: u32,
}

pub struct CleanResult {
    pub content: String,
    pub counts: CleanCounts,
}

fn is_separator(c: char) -> bool {
    matches!(c, '\u{0640}' | '\u{200c}' | '\u{200b}' | ' ' | '.' | '_')
        || ('\u{064b}'..='\u{0652}').contains(&c)
        || c == '\u{0670}'
}

pub fn flatten(s: &str) -> String {
    s.chars().filter(|c| !is_separator(*c)).collect()
}

pub fn clean_srt(content: &str) -> CleanResult {
    let normalized = content.replace("\r\n", "\n");
    let mut counts = CleanCounts::default();
    let mut out_blocks: Vec<String> = Vec::new();

    for block in normalized.split("\n\n") {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut lines = trimmed.lines();
        let _index_line = lines.next().unwrap_or_default();
        let timing_line = lines.next().unwrap_or_default();
        if !timing_line.contains("-->") {
            out_blocks.push(block.to_string());
            continue;
        }

        let mut kept: Vec<String> = Vec::new();
        for tl in lines {
            let flat = flatten(tl).to_lowercase();
            if AD_FLAT.iter().any(|fp| flat.contains(fp.as_str())) {
                counts.ads_removed += 1;
                continue;
            }
            if BRAND_FLAT.iter().any(|fp| flat.contains(fp.as_str())) {
                counts.brand_lines += 1;
                continue;
            }
            kept.push(tl.to_string());
        }

        if kept.is_empty() {
            continue;
        }

        let idx = out_blocks.len() + 1;
        let mut block_out = String::new();
        block_out.push_str(&idx.to_string());
        block_out.push('\n');
        block_out.push_str(timing_line);
        for tl in &kept {
            block_out.push('\n');
            block_out.push_str(tl);
        }
        out_blocks.push(block_out);
    }

    let content_out = out_blocks.join("\n\n");
    CleanResult { content: content_out, counts }
}

pub fn clean_srt_file(path: &Path) -> anyhow::Result<CleanCounts> {
    if path.extension().map(|e| e != "srt").unwrap_or(true) {
        return Ok(CleanCounts::default());
    }
    let content = std::fs::read_to_string(path)?;
    let result = clean_srt(&content);
    if result.counts.ads_removed > 0 || result.counts.brand_lines > 0 {
        std::fs::write(path, result.content.as_bytes())?;
    }
    Ok(result.counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_strips_obfuscation_and_diacritics() {
        assert_eq!(flatten("ا‌مـپـایـر‌بـسـت‌تـی‌و‌ی"), "امپایربستتیوی");
        assert_eq!(flatten("اِ م.پـا_یـر"), "امپایر");
        assert_eq!(flatten("اِمپایر بِست تی وی"), "امپایربستتیوی");
        assert_eq!(
            flatten("دیجی موویز را در شبکه های اجتماعی دنبال کنید"),
            "دیجیموویزرادرشبکههایاجتماعیدنبالکنید"
        );
    }

    #[test]
    fn clean_srt_deletes_ad_line_and_drops_empty_cue() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nسلام\n\n\
                     2\n00:00:03,000 --> 00:00:04,000\nارائه ای از وبسایت دیجی موویز\n.:: DigiMoviez.Com ::.\n\n\
                     3\n00:00:05,000 --> 00:00:06,000\nخداحافظ\n";
        let result = clean_srt(input);
        assert_eq!(result.counts.ads_removed, 2);
        assert_eq!(result.counts.brand_lines, 0);
        assert!(result.content.contains("سلام"));
        assert!(result.content.contains("خداحافظ"));
        assert!(!result.content.contains("DigiMoviez"));
    }

    #[test]
    fn clean_srt_deletes_brand_line_keeps_dialogue() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nلوئیس، بیا دیگه\nا‌مـپـایـر‌بـسـت‌تـی‌و‌ی\n";
        let result = clean_srt(input);
        assert_eq!(result.counts.ads_removed, 0);
        assert_eq!(result.counts.brand_lines, 1);
        assert!(result.content.contains("لوئیس، بیا دیگه"));
        assert!(!result.content.contains("امپایر"));
    }

    #[test]
    fn clean_srt_deletes_inline_brand_whole_line_and_renumbers() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nخوشحالم\n\n\
                     2\n00:00:03,000 --> 00:00:04,000\nاین همون سلبریتیه که دوستش داری ا مپایر بست تی وی\n\n\
                     3\n00:00:05,000 --> 00:00:06,000\nخداحافظ\n";
        let result = clean_srt(input);
        assert_eq!(result.counts.brand_lines, 1);
        assert_eq!(result.counts.ads_removed, 0);
        assert!(!result.content.contains("سلبریتیه"));
        assert!(result.content.contains("خوشحالم"));
        assert!(result.content.contains("خداحافظ"));
        assert!(result.content.contains("2\n00:00:05,000"));
    }

    #[test]
    fn clean_srt_matches_kasra_brand_variant() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nوقتی اِمپایر بِست تی وی بازی میکنه\n";
        let result = clean_srt(input);
        assert_eq!(result.counts.brand_lines, 1);
        assert_eq!(result.content, "");
    }

    #[test]
    fn clean_srt_matches_short_brand_prefix() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nتبلیغ امپایربست\n";
        let result = clean_srt(input);
        assert_eq!(result.counts.brand_lines, 1);
        assert_eq!(result.content, "");
    }

    #[test]
    fn clean_srt_matches_ebtv_ad() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nEBTV\n\n\
                     2\n00:00:03,000 --> 00:00:04,000\nفیلم را از E.B.T.V دنبال کنید\n";
        let result = clean_srt(input);
        assert_eq!(result.counts.ads_removed, 2);
        assert_eq!(result.content, "");
    }

    #[test]
    fn clean_srt_handles_windows_line_endings() {
        let input = "1\r\n00:00:01,000 --> 00:00:02,000\r\nخوشحالم\r\n\r\n\
                     2\r\n00:00:03,000 --> 00:00:04,000\r\nارائه ای از وبسایت دیجی موویز\r\n";
        let result = clean_srt(input);
        assert_eq!(result.counts.ads_removed, 1);
        assert!(result.content.contains("خوشحالم"));
        assert!(!result.content.contains("دیجی موویز"));
    }

    #[test]
    fn clean_srt_file_skips_non_srt() {
        let dir = std::env::temp_dir().join("sub_rs_clean_test");
        std::fs::create_dir_all(&dir).unwrap();
        let ass = dir.join("m.ass");
        let original = "Dialogue: 0,0:00:01,0:00:02,Default,,0,0,0,,ا‌مـپـایـر‌بـسـت‌تـی‌و‌ی".to_string();
        std::fs::write(&ass, &original).unwrap();
        let counts = clean_srt_file(&ass).unwrap();
        assert_eq!(counts, CleanCounts::default());
        let after = std::fs::read_to_string(&ass).unwrap();
        assert_eq!(after, original);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clean_srt_file_rewrites_srt_with_ads() {
        let dir = std::env::temp_dir().join("sub_rs_clean_file_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.fa.srt");
        std::fs::write(&path, "1\n00:00:01,000 --> 00:00:02,000\nجهت اطلاع از جدیدترین آدرس سایت دیجی موویز\n").unwrap();
        let counts = clean_srt_file(&path).unwrap();
        assert_eq!(counts.ads_removed, 1);
        assert_eq!(counts.brand_lines, 0);
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(!after.contains("دیجی موویز"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clean_srt_file_leaves_clean_srt_untouched() {
        let dir = std::env::temp_dir().join("sub_rs_clean_clean_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.fa.srt");
        let content = "1\n00:00:01,000 --> 00:00:02,000\nسلام\n";
        std::fs::write(&path, content).unwrap();
        let counts = clean_srt_file(&path).unwrap();
        assert_eq!(counts, CleanCounts::default());
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("سلام"));
        std::fs::remove_dir_all(&dir).ok();
    }
}