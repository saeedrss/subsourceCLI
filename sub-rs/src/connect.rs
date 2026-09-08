use anyhow::{anyhow, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const API_BASE: &str = "https://api.subsource.net/api/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum Mode {
    Direct,
    System,
    Manual,
    Cloudflare,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub mode: Mode,
    pub manual_proxy: Option<String>,
    pub worker_url: Option<String>,
}

impl Connection {
    pub fn direct() -> Self {
        Connection {
            mode: Mode::Direct,
            manual_proxy: None,
            worker_url: None,
        }
    }

    pub fn from_parts(
        mode: Option<Mode>,
        manual_proxy: Option<String>,
        worker_url: Option<String>,
    ) -> Self {
        let mode = match mode {
            Some(m) => m,
            None if manual_proxy.is_some() => Mode::Manual,
            None => Mode::Direct,
        };
        Connection {
            mode,
            manual_proxy,
            worker_url,
        }
    }

    pub fn api_base(&self) -> String {
        match self.mode {
            Mode::Cloudflare => match &self.worker_url {
                Some(w) => format!("{}/api/v1", w.trim_end_matches('/')),
                None => API_BASE.to_string(),
            },
            _ => API_BASE.to_string(),
        }
    }

    pub fn builder(&self) -> Result<reqwest::blocking::ClientBuilder> {
        let mut builder = reqwest::blocking::ClientBuilder::new();
        match self.mode {
            Mode::Direct | Mode::Cloudflare => {
                builder = builder.no_proxy();
            }
            Mode::System => {}
            Mode::Manual => {
                let url = self
                    .manual_proxy
                    .as_deref()
                    .ok_or_else(|| anyhow!("Manual proxy selected but no proxy URL configured"))?;
                builder = builder.proxy(reqwest::Proxy::all(url)?);
            }
        }
        Ok(builder)
    }

    pub fn build_reqwest(
        &self,
        timeout: Duration,
        user_agent: &str,
    ) -> Result<reqwest::blocking::Client> {
        Ok(self.builder()?.timeout(timeout).user_agent(user_agent).build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ValueEnum;

    #[test]
    fn mode_parses_from_clap_lowercase() {
        assert_eq!(Mode::from_str("direct", true).unwrap(), Mode::Direct);
        assert_eq!(Mode::from_str("system", true).unwrap(), Mode::System);
        assert_eq!(Mode::from_str("manual", true).unwrap(), Mode::Manual);
        assert_eq!(
            Mode::from_str("cloudflare", true).unwrap(),
            Mode::Cloudflare
        );
    }

    #[test]
    fn mode_parses_from_serde_lowercase() {
        assert_eq!(
            serde_json::from_str::<Mode>("\"manual\"").unwrap(),
            Mode::Manual
        );
        assert_eq!(
            serde_json::from_str::<Mode>("\"cloudflare\"").unwrap(),
            Mode::Cloudflare
        );
    }

    #[test]
    fn from_parts_defaults_to_direct() {
        let c = Connection::from_parts(None, None, None);
        assert_eq!(c.mode, Mode::Direct);
    }

    #[test]
    fn from_parts_proxy_implies_manual() {
        let c = Connection::from_parts(None, Some("http://1.2.3.4:8080".into()), None);
        assert_eq!(c.mode, Mode::Manual);
        assert_eq!(c.manual_proxy.as_deref(), Some("http://1.2.3.4:8080"));
    }

    #[test]
    fn from_parts_explicit_mode_wins() {
        let c = Connection::from_parts(Some(Mode::System), Some("http://1.2.3.4:8080".into()), None);
        assert_eq!(c.mode, Mode::System);
    }

    #[test]
    fn api_base_cloudflare_uses_worker() {
        let c = Connection::from_parts(
            Some(Mode::Cloudflare),
            None,
            Some("https://subsource-proxy.workers.dev".into()),
        );
        assert_eq!(c.api_base(), "https://subsource-proxy.workers.dev/api/v1");
    }

    #[test]
    fn api_base_trailing_slash_trimmed() {
        let c = Connection::from_parts(
            Some(Mode::Cloudflare),
            None,
            Some("https://subsource-proxy.workers.dev/".into()),
        );
        assert_eq!(c.api_base(), "https://subsource-proxy.workers.dev/api/v1");
    }

    #[test]
    fn api_base_direct_uses_subsource() {
        let c = Connection::direct();
        assert_eq!(c.api_base(), API_BASE);
    }

    #[test]
    fn build_reqwest_manual_without_proxy_errors() {
        let c = Connection::from_parts(Some(Mode::Manual), None, None);
        assert!(c.build_reqwest(Duration::from_secs(5), "test").is_err());
    }

    #[test]
    fn build_reqwest_direct_builds_client() {
        let c = Connection::direct();
        assert!(c.build_reqwest(Duration::from_secs(5), "test").is_ok());
    }
}