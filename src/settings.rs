use std::env;

use lazy_static::lazy_static;

use std::error::Error;

use config::{Config, Environment, File};
use serde_derive::Deserialize;

use std::convert::TryInto;

use log::{error, info};

lazy_static! {
    pub static ref SETTINGS: Settings = Settings::new().unwrap();
}



#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct Settings {
    pub debug: bool,
    pub log_level: String,
    pub hostname: String,
    pub http_port: u16,
    pub socket_port: u16,
    pub service_discovery_type: String,
    pub static_service_list: Vec<String>,
}

impl From<Config> for Settings {
    fn from(config: Config) -> Self {
        let debug = config.get_bool("is_debug").unwrap_or(false);
        let log_level = config.get::<String>("log_level").unwrap_or(String::from("INFO"));
        let hostname = config.get::<String>("fairy_hostname").unwrap_or(hostname::get().unwrap().into_string().unwrap());
        let http_port = config.get::<u16>("http_port").unwrap_or(8080);
        let socket_port = config.get::<u16>("socket_port").unwrap_or(19090);
        let service_discovery_type = 
            config.get_string("service_discovery_type").unwrap_or(String::from("static"));
        let static_service_list = if service_discovery_type == "static" {
            config.get_string("static_service_list")
                .unwrap_or(String::from("static")).split(",").map(String::from).collect()
        } else {
            Vec::new()
        };
        let settings = Settings {
            debug,
            log_level,
            hostname,
            http_port,
            socket_port,
            service_discovery_type,
            static_service_list
        };
        info!("Settings loaded {:?}", settings);
        settings
    }
}

impl Settings {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let config_filename = env::var("FAIRY_CONFIG").unwrap_or_else(|_| "fairy_config".into());
        let config_builder:Result<Settings, _> = Config::builder()
            .add_source(File::with_name(config_filename.as_str()))
            .add_source(Environment::default())
            .build()?
            .try_into();
        match config_builder {
            Ok(settings) => Ok(settings),
            Err(err) => {
                eprintln!("Failed to parse settings: {}", err);
                Err(err.into())
            }
        }
    }
}