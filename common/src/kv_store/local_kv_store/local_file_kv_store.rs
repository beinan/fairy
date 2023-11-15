use std::error::Error;
use std::io::ErrorKind;

use bytes::Bytes;
use log::trace;

use crate::kv_store::Key;
use crate::settings::local_kv_options::LocalFileKVStoreOptions;

pub struct LocalFileKVStore {
    options: LocalFileKVStoreOptions,
}

impl LocalFileKVStore {
    pub fn new(options: LocalFileKVStoreOptions) -> LocalFileKVStore {
        LocalFileKVStore { options }
    }

    pub async fn put<K: Key>(&self, id: K, buf: Bytes) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = self.data_path(id);
        trace!("Start writing data to {}", path.clone());
        let file = match monoio::fs::File::create(&path).await {
            Ok(file) => file,
            Err(error) => match error.kind() {
                ErrorKind::NotFound => {
                    let path = std::path::Path::new(path.as_str());
                    let prefix = path.parent().unwrap();
                    match std::fs::create_dir_all(prefix) {
                        Ok(_) => match monoio::fs::File::create(&path).await {
                            Ok(f) => f,
                            Err(e) => return Err(e.to_string().into()),
                        },
                        Err(e) => return Err(e.to_string().into()),
                    }
                }
                _other_error => {
                    return Err(error.to_string().into());
                }
            },
        };

        let (res, _) = file.write_all_at(buf, 0).await;
        res?;
        file.close().await?;
        trace!("Write data to file {}", path);
        Ok(())
    }

    pub async fn get<K: Key>(&self, id: K) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let path = self.data_path(id);
        let f = monoio::fs::File::open(&path).await?;
        let metadata = std::fs::metadata(&path)?;
        let file_size = metadata.len();
        let buf = vec![0; file_size as usize];
        let (res, buf) = f.read_exact_at(buf, 0).await;
        res?;
        f.close().await?;
        trace!("Read data from file {}", path);
        Ok(buf)
    }

    fn data_path<K: Key>(&self, id: K) -> String {
        let path = format!(
            "{}/{}/{}",
            self.options.root_path,
            id.short_hash() % self.options.num_bucket,
            id.filename()
        );
        path
    }
}
