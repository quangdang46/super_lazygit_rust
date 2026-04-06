use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct ThreadSafeMap<K: Eq + Hash, V> {
    inner: RwLock<HashMap<K, V>>,
}

impl<K: Eq + Hash, V> ThreadSafeMap<K, V> {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let guard = self.inner.read().unwrap();
        guard.get(key).cloned()
    }

    pub fn set(&self, key: K, value: V) {
        let mut guard = self.inner.write().unwrap();
        guard.insert(key, value);
    }

    pub fn delete(&self, key: &K) {
        let mut guard = self.inner.write().unwrap();
        guard.remove(key);
    }

    pub fn keys(&self) -> Vec<K>
    where
        K: Clone,
    {
        let guard = self.inner.read().unwrap();
        guard.keys().cloned().collect()
    }

    pub fn values(&self) -> Vec<V>
    where
        V: Clone,
    {
        let guard = self.inner.read().unwrap();
        guard.values().cloned().collect()
    }

    pub fn len(&self) -> usize {
        let guard = self.inner.read().unwrap();
        guard.len()
    }

    pub fn clear(&self) {
        let mut guard = self.inner.write().unwrap();
        *guard = HashMap::new();
    }

    pub fn is_empty(&self) -> bool {
        let guard = self.inner.read().unwrap();
        guard.is_empty()
    }

    pub fn has(&self, key: &K) -> bool {
        let guard = self.inner.read().unwrap();
        guard.contains_key(key)
    }
}

impl<K: Eq + Hash, V> Default for ThreadSafeMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_is_empty() {
        let map: ThreadSafeMap<i32, String> = ThreadSafeMap::new();
        assert!(map.is_empty());
    }

    #[test]
    fn test_set_and_get() {
        let map: ThreadSafeMap<i32, String> = ThreadSafeMap::new();
        map.set(1, "one".to_string());
        assert_eq!(map.get(&1), Some("one".to_string()));
        assert_eq!(map.get(&2), None);
    }

    #[test]
    fn test_delete() {
        let map: ThreadSafeMap<i32, String> = ThreadSafeMap::new();
        map.set(1, "one".to_string());
        assert!(map.has(&1));
        map.delete(&1);
        assert!(!map.has(&1));
    }

    #[test]
    fn test_keys_and_values() {
        let map: ThreadSafeMap<i32, String> = ThreadSafeMap::new();
        map.set(1, "one".to_string());
        map.set(2, "two".to_string());

        let mut keys = map.keys();
        keys.sort();
        assert_eq!(keys, vec![1, 2]);

        let mut values = map.values();
        values.sort();
        assert_eq!(values, vec!["one".to_string(), "two".to_string()]);
    }

    #[test]
    fn test_len() {
        let map: ThreadSafeMap<i32, String> = ThreadSafeMap::new();
        assert_eq!(map.len(), 0);
        map.set(1, "one".to_string());
        assert_eq!(map.len(), 1);
        map.set(2, "two".to_string());
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_clear() {
        let map: ThreadSafeMap<i32, String> = ThreadSafeMap::new();
        map.set(1, "one".to_string());
        map.set(2, "two".to_string());
        assert_eq!(map.len(), 2);
        map.clear();
        assert!(map.is_empty());
    }

    #[test]
    fn test_has() {
        let map: ThreadSafeMap<i32, String> = ThreadSafeMap::new();
        map.set(1, "one".to_string());
        assert!(map.has(&1));
        assert!(!map.has(&2));
    }

    #[test]
    fn test_overwrite() {
        let map: ThreadSafeMap<i32, String> = ThreadSafeMap::new();
        map.set(1, "one".to_string());
        map.set(1, "ONE".to_string());
        assert_eq!(map.get(&1), Some("ONE".to_string()));
        assert_eq!(map.len(), 1);
    }
}
