use image::RgbaImage;
use std::path::Path;

#[derive(Clone)]
pub struct AdData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

fn generate_placeholder(w: u32, h: u32) -> AdData {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let r = ((x as f32 / w as f32) * 200.0) as u8 + 55;
            let g = ((y as f32 / h as f32) * 200.0) as u8 + 55;
            let b = 100;
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    AdData { rgba, width: w, height: h }
}

fn get_ads_dir() -> std::path::PathBuf {
    let exe = std::env::current_exe().ok();
    if let Some(p) = exe.and_then(|p| p.parent().map(|p| p.join("ads"))) {
        if p.exists() {
            return p;
        }
    }
    std::path::PathBuf::from("ads")
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

    if ads.is_empty() {
        let placeholder = generate_placeholder(300, 100);
        let _ = save_placeholder_png(&ads_dir, &placeholder);
        ads.push(placeholder);
        ads.push(generate_placeholder(250, 80));
    }

    ads
}

fn save_placeholder_png(ads_dir: &Path, ad: &AdData) -> Result<(), Box<dyn std::error::Error>> {
    let img = RgbaImage::from_raw(ad.width, ad.height, ad.rgba.clone())
        .ok_or("failed to create image")?;
    std::fs::create_dir_all(ads_dir)?;
    img.save(ads_dir.join("placeholder.png"))?;
    Ok(())
}
