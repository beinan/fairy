use std::env;

use lazy_static::lazy_static;


use config::{Config, ConfigError, File};
use serde_derive::Deserialize;

lazy_static! {
    pub static ref SETTINGS: Settings = Settings::new().unwrap();
}



#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct Settings {
    pub debug: bool,
    pub http_port: u16,
    pub socket_port: u16,
    pub service_discovery_type: String,
    pub static_service_list: Vec<String>,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let config_filename = env::var("FAIRY_CONFIG").unwrap_or_else(|_| "fairy_config".into());
        let s = Config::builder()
            .add_source(File::with_name(config_filename.as_str()))
            .build()?;
        s.try_deserialize()
    }
}