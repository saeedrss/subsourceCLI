use std::path::PathBuf;

#[derive(Clone)]
pub struct AdData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

fn get_ads_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("subsource").join("ads")
}

pub fn load_ads() -> Vec<AdData> {
    let ads_dir = get_ads_dir();
    let mut ads: Vec<AdData> = Vec::new();

    if ads_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&ads_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("png") {
                    if let Ok(bytes) = std::fs::read(&path) {
                        if let Ok(img) = image::load_from_memory(&bytes) {
                            let rgba = img.to_rgba8();
                            let (w, h) = rgba.dimensions();
                            ads.push(AdData {
                                rgba: rgba.into_raw(),
                                width: w,
                                height: h,
                            });
                        }
                    }
                }
            }
        }
    }

    ads
}

pub fn ads_signature() -> String {
    let ads_dir = get_ads_dir();
    let mut sig = String::new();
    if ads_dir.exists() {
        let mut names: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&ads_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("png") {
                    let meta = path.metadata().map(|m| m.len()).unwrap_or(0);
                    names.push(format!(
                        "{}|{}|",
                        path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                        meta
                    ));
                }
            }
        }
        names.sort();
        for n in names {
            sig.push_str(&n);
        }
    }
    sig
}
