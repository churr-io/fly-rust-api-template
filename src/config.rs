use std::path::PathBuf;
use std::time::Duration;

use confique::{Config, Error};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumString};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, EnumString, AsRefStr)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum Env {
    Dev,
    Beta,
    Prod,
}

#[derive(Clone, Debug, confique::Config, Serialize)]
pub struct AppConfig {
    #[config(env = "APP_ENV", default = "dev")]
    pub env: Env,
    #[config(env = "APP_DOMAIN", default = "localhost")]
    pub domain: String,
    #[config(env = "LOG_LEVEL", default = "info")]
    pub log_level: String,
    #[config(nested)]
    pub http: HttpConfig,
    #[config(env = "CARGO_PKG_VERSION")]
    pub version: String
}

#[derive(Clone, Debug, Config, Serialize)]
pub struct HttpConfig {
    #[config(env = "HTTP_ADDRESS", default = "127.0.0.1")]
    pub address: String,
    #[config(env = "HTTP_PORT", default = 8080)]
    pub port: u16,
    #[config(default = "30s", deserialize_with = crate::util::serde_duration::deserialize)]
    #[serde(with = "crate::util::serde_duration")]
    pub timeout: Duration,
    #[config(nested)]
    pub rate_limit: HttpRateLimitConfig,
    #[config(default = false)]
    pub proxy_proto_enabled: bool,
    #[config(default = "30s", deserialize_with = crate::util::serde_duration::deserialize)]
    #[serde(with = "crate::util::serde_duration")]
    pub conn_read_timeout: Duration,
    #[config(default = "30s", deserialize_with = crate::util::serde_duration::deserialize)]
    #[serde(with = "crate::util::serde_duration")]
    pub conn_write_timeout: Duration,
}

#[derive(Clone, Debug, Config, Serialize)]
pub struct HttpRateLimitConfig {
    #[config(default = "1m", deserialize_with = crate::util::serde_duration::deserialize)]
    #[serde(with = "crate::util::serde_duration")]
    pub refill_interval: Duration,
    #[config(default = 10)]
    pub max_burst_limit: usize,
}

impl AppConfig {
    pub fn load(config_path: Option<impl Into<PathBuf>>) -> Result<Self, Error> {
        let mut config = AppConfig::builder();
        if let Some(config_path) = config_path {
            config = config.file(config_path);
        }
        Ok(config.env().load()?)
    }
}
