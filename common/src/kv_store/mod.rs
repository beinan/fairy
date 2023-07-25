use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub mod local_kv_store;

// #[async_trait]
// pub trait KVStore<K: Key, V: Value> {
//     async fn put(
//         &self,
//         id: K,
//         data: V,
//     ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
//     async fn get(
//         &self,
//         id: K,
//         data: V
//     ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
// }

pub trait Key: Send {
    fn short_hash(&self) -> u16;
    fn filename(&self) -> String;
}

impl Key for String {
    fn short_hash(&self) -> u16 {
        let mut s = DefaultHasher::new();
        self.hash(&mut s);
        s.finish() as u16
    }

    fn filename(&self) -> String {
        self.clone()
    }
}

pub trait Value: Send {}

impl Value for Vec<u8> {}
