use anyhow::Result;
use figment::providers::{Format, Serialized};
use figment::{
    providers::{Env, Yaml},
    Figment,
};
use platform_dirs::AppDirs;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};
use tracing::info;

static APP_NAME: &str = "swandns";

pub async fn load_config<T: Serialize + DeserializeOwned + Default>(
    conf_name: &str,
    path: Option<PathBuf>,
) -> Result<T> {
    let path: PathBuf = match path {
        Some(path) => path,
        None => confy::get_configuration_file_path(APP_NAME, conf_name)?,
    };
    info!("Loading config from {:?}", path);
    let defaults: T = Default::default();
    let cfg: T = Figment::from(Serialized::defaults(defaults))
        .merge(Yaml::file(path))
        .merge(Env::prefixed("SWANDNS_"))
        .extract()?;
    Ok(cfg)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    pub bind: Option<String>,
    pub dns_port: u16,
    pub api_port: u16,
    pub nameservers: Vec<String>,
    pub zones: Vec<ZoneConfig>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        let data_dir = AppDirs::new(Some("swandns"), false)
            .map_or(env::temp_dir(), |app_dirs| app_dirs.data_dir);
        return Self {
            data_dir,
            db_file: PathBuf::from("swandns.db"),
            bind: None,
            dns_port: 1053,
            api_port: 8080,
            nameservers: vec![],
            zones: vec![],
        };
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneConfig {
    pub name: String,
    #[serde(default)]
    pub records: Vec<RecordConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordConfig {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub default_server_url: Option<String>,
    pub default_bind: Option<String>,
    pub default_protocol: Option<String>,
    pub records: Vec<ClientRecordConfig>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            default_server_url: None,
            default_bind: None,
            default_protocol: None,
            records: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRecordConfig {
    pub server_url: Option<String>,
    pub name: String,
    pub bind: Option<String>,
    pub protocol: Option<String>,
}
