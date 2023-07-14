use std::hash::{Hash, Hasher};

mod local_kv_store;

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

pub trait Key : Send{
    fn hashcode(&self) -> u64;
    fn filename(&self) -> String;
}

impl Key for String {
    fn hashcode(&self) -> u64 {
        self.hashcode()
    }

    fn filename(&self) -> String {
        self.clone()
    }
}

pub trait Value : Send {}

impl Value for Vec<u8> {}
