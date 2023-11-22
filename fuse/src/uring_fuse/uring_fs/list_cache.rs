use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub struct ListStatusCache {
    cache: HashMap<String, Vec<String>>,
}

impl ListStatusCache {
    pub fn new() -> Self {
        ListStatusCache {
            cache: HashMap::new(),
        }
    }

    pub fn get(&self, path: &str) -> Option<&Vec<String>> {
        self.cache.get(path)
    }

    pub fn append(&mut self, path: String, new_file: String) {
        match self.cache.entry(path) {
            Entry::Occupied(mut entry) => entry.get_mut().push(new_file),
            Entry::Vacant(entry) => {
                entry.insert(vec![new_file]);
            }
        }
    }
}
