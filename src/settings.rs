use lazy_static::lazy_static;


use config::{Config, ConfigError, File};
use serde_derive::Deserialize;
use std::env;

lazy_static! {
    pub static ref SETTINGS: Settings = Settings::new().unwrap();
}



#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct Settings {
    pub debug: bool,
    pub http_port: u16,
    pub socket_port: u16,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        let s = Config::builder()
            .add_source(File::with_name("fairy_config"))
            .build()?;
        // println!("load settings: {:?}", s);
        s.try_deserialize()
    }
}