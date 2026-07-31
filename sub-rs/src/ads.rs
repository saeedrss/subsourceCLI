use serde::Deserialize;

#[derive(Clone)]
pub struct AdData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

const API_URL: &str = "https://api.github.com/repos/saeedrss/subsourceCLI/contents/ads";
const RAW_URL: &str = "https://raw.githubusercontent.com/saeedrss/subsourceCLI/master/ads";

#[derive(Deserialize)]
struct RepoEntry {
    name: String,
    download_url: Option<String>,
}

pub fn fetch_ads(proxy: Option<&str>) -> Vec<AdData> {
    let client = http_client(proxy);
    let entries = match list_ads_folder(&client) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut ads: Vec<AdData> = Vec::new();
    for entry in entries {
        if !entry.name.to_lowercase().ends_with(".png") {
            continue;
        }
        let url = entry
            .download_url
            .unwrap_or_else(|| format!("{}/{}", RAW_URL, entry.name));
        if let Some(bytes) = client.get(&url).send().ok().and_then(|r| r.bytes().ok()) {
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

    ads
}

fn http_client(proxy: Option<&str>) -> reqwest::blocking::Client {
    let mut builder = reqwest::blocking::Client::builder()
        .user_agent("sub-rs")
        .timeout(std::time::Duration::from_secs(10));
    if let Some(p) = proxy {
        if !p.is_empty() {
            if let Ok(proxy) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy);
            }
        }
    }
    builder.build().unwrap_or_else(|_| reqwest::blocking::Client::new())
}

fn list_ads_folder(client: &reqwest::blocking::Client) -> Result<Vec<RepoEntry>, String> {
    let resp = client
        .get(API_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API status {}", resp.status()));
    }
    resp.json::<Vec<RepoEntry>>().map_err(|e| e.to_string())
}
