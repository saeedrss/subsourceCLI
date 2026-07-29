use serde::Deserialize;

const RELEASES_URL: &str = "https://api.github.com/repos/saeedrss/subsourceCLI/releases/latest";
const TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub body: String,
}

fn parse_version(v: &str) -> Vec<u32> {
    v.trim_start_matches('v')
        .split('.')
        .filter_map(|p| p.parse::<u32>().ok())
        .collect()
}

pub fn check_for_update(current: &str, proxy: Option<&str>) -> Option<UpdateInfo> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .user_agent("sub-rs/1.0");
    if let Some(p) = proxy {
        if !p.is_empty() {
            if let Ok(proxy) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy);
            }
        }
    }
    let client = match builder.build() {
        Ok(c) => c,
        Err(_) => return None,
    };

    let resp = match client.get(RELEASES_URL).send() {
        Ok(r) => r,
        Err(_) => return None,
    };

    let release: GitHubRelease = match resp.json() {
        Ok(r) => r,
        Err(_) => return None,
    };

    let current_ver = parse_version(current);
    let latest_ver = parse_version(&release.tag_name);

    if latest_ver > current_ver {
        Some(UpdateInfo {
            latest_version: release.tag_name,
            body: release.body.unwrap_or_default(),
        })
    } else {
        None
    }
}
