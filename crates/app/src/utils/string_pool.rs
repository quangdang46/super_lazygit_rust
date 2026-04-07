// Ported from ./references/lazygit-master/pkg/utils/string_pool.go

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct StringPool {
    map: RwLock<HashMap<String, Arc<String>>>,
}

impl StringPool {
    pub fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
        }
    }

    pub fn add(&self, s: String) -> Arc<String> {
        let mut map = self.map.write().unwrap();
        if let Some(existing) = map.get(&s) {
            return existing.clone();
        }
        let arc = Arc::new(s.clone());
        map.insert(s, arc.clone());
        arc
    }
}

impl Default for StringPool {
    fn default() -> Self {
        Self::new()
    }
}
