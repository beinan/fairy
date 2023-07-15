use std::error::Error;

use crate::kv_store::{Key, Value};
use crate::settings::local_kv_options::LocalFileKVStoreOptions;

pub struct LocalFileKVStore {
    options: LocalFileKVStoreOptions,
}

impl LocalFileKVStore {
    pub fn new(options: LocalFileKVStoreOptions) -> LocalFileKVStore {
        LocalFileKVStore { options }
    }

    pub async fn put<K: Key>(&self, id: K, buf: Vec<u8>) -> Result<(), Box<dyn Error + Send + Sync>>
    {
        let path = self.data_path(id);
        let f = monoio::fs::File::create(path).await?;
        let (res, _) = f.write_all_at(buf, 0).await;
        res?;
        f.close().await?;
        Ok(())
    }

    pub async fn get<K: Key>(&self, id: K, buf: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>>
    {
        let path = self.data_path(id);
        let f = monoio::fs::File::open(path).await?;
        // let metadata = std::fs::metadata(path)?;
        // let file_size = metadata.len();
        let (res, buf) = f.read_exact_at(buf, 0).await;
        res?;
        f.close().await?;
        Ok(buf)
    }

    fn data_path<K: Key>(&self, id: K) -> String {
        let path = format!(
            "{}/{}/{}",
            self.options.root_path,
            id.hashcode() % self.options.num_bucket as u64,
            id.filename()
        );
        path
    }
}
