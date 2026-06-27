use anyhow::Result;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

const API_BASE: &str = "https://api.subsource.net/api/v1";
const REQUEST_DELAY_SECS: f64 = 1.0;

#[derive(Debug, Deserialize)]
struct ApiData<T> {
    data: Option<Vec<T>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Movie {
    #[serde(default)]
    pub movie_id: Option<i32>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub alternate_title: Option<String>,
    #[serde(rename = "type")]
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub season: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleEntry {
    #[serde(default)]
    pub subtitle_id: Option<i32>,
    #[serde(default)]
    pub release_info: Option<Vec<String>>,
    #[serde(default)]
    pub commentary: Option<String>,
    #[serde(default)]
    pub rating: Option<Rating>,
    #[serde(default)]
    pub downloads: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Rating {
    #[serde(default)]
    pub good: i32,
    #[serde(default)]
    pub total: i32,
}

pub struct Client {
    http: reqwest::blocking::Client,
    api_key: String,
}

fn parse_response<T: serde::de::DeserializeOwned>(text: &str) -> Result<Vec<T>> {
    if let Ok(v) = serde_json::from_str::<Vec<T>>(text) {
        return Ok(v);
    }
    let wrapped: Result<ApiData<T>, _> = serde_json::from_str(text);
    match wrapped {
        Ok(w) => Ok(w.data.unwrap_or_default()),
        Err(e) => {
            // ; pony: debug API response format
            let preview = text.chars().take(500).collect::<String>();
            eprintln!("  [DEBUG] Parse error: {}\n  [DEBUG] Response: {}", e, preview);
            Err(anyhow::anyhow!("API response format unexpected"))
        }
    }
}

fn rate_limit() {
    std::thread::sleep(Duration::from_secs_f64(REQUEST_DELAY_SECS));
}

impl Client {
    pub fn new(api_key: String, proxy: Option<String>) -> Result<Self> {
        let mut builder = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
        if let Some(p) = proxy {
            builder = builder.proxy(reqwest::Proxy::all(&p)?);
        }
        Ok(Client {
            http: builder.build()?,
            api_key,
        })
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert("X-API-Key", self.api_key.parse().unwrap());
        h.insert("Accept", "application/json".parse().unwrap());
        h
    }

    pub fn search_movie(&self, query: &str, year: Option<&str>) -> Result<Vec<Movie>> {
        rate_limit();
        let mut params = vec![("searchType", "text"), ("q", query), ("type", "all")];
        if let Some(y) = year {
            params.push(("year", y));
        }
        let resp = self
            .http
            .get(format!("{}/movies/search", API_BASE))
            .headers(self.headers())
            .query(&params)
            .send()?;
        let text = resp.error_for_status()?.text()?;
        parse_response(&text)
    }

    pub fn get_subtitles(&self, movie_id: &str, lang: &str) -> Result<Vec<SubtitleEntry>> {
        rate_limit();
        let params = vec![
            ("movieId", movie_id),
            ("language", lang),
            ("sort", "rating"),
            ("limit", "100"),
        ];
        let resp = self
            .http
            .get(format!("{}/subtitles", API_BASE))
            .headers(self.headers())
            .query(&params)
            .send()?;
        let text = resp.error_for_status()?.text()?;
        parse_response(&text)
    }

    pub fn download_zip(&self, subtitle_id: &str, output: &Path) -> Result<()> {
        rate_limit();
        let resp = self
            .http
            .get(format!("{}/subtitles/{}/download", API_BASE, subtitle_id))
            .headers(self.headers())
            .send()?;
        let bytes = resp.error_for_status()?.bytes()?;
        std::fs::write(output, &bytes)?;
        Ok(())
    }
}
