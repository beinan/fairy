use config::Config;
use log::info;
use serde::Deserialize;

use crate::settings::FromConfig;

#[derive(Clone, Debug)]
#[allow(unused)]
pub struct LocalFileKVStoreOptions {
    pub root_path: String,
    pub num_bucket: u16,
    pub chuck_size: u32,
}

impl FromConfig for LocalFileKVStoreOptions {
    fn from_with_prefix(prefix: &str, config: &Config) -> Self {
        let root_path = get_config(
            config,
            prefix,
            "local_kv_root_path",
            String::from("/tmp/fairy_store"),
        );
        let num_bucket = get_config(config, prefix, "local_kv_num_bucket", 1024);
        let chuck_size = get_config(config, prefix, "local_kv_chunk_size", 128 * 1024);

        let options = LocalFileKVStoreOptions {
            root_path,
            num_bucket,
            chuck_size,
        };
        info!("LocalFileKVStoreOptions loaded {:?}", options);
        options
    }
}

fn get_config<'a, T>(config: &Config, prefix: &str, key: &str, default: T) -> T
where
    T: Deserialize<'a>,
{
    config
        .get::<T>(format!("{}.{}", prefix, key).as_str())
        .unwrap_or(default)
}
