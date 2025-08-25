use std::collections::HashMap;

#[derive(Default)]
pub struct LruCache<K, V> {
    cap: usize,
    map: HashMap<K, (V, u64)>,
    tick: u64,
}

impl<K: std::hash::Hash + Eq + Clone, V> LruCache<K, V> {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            map: HashMap::new(),
            tick: 0,
        }
    }

    pub fn get(&mut self, k: &K) -> Option<&V> {
        if let Some((v, t)) = self.map.get_mut(k) {
            self.tick += 1;
            *t = self.tick;
            Some(v)
        } else {
            None
        }
    }

    pub fn put(&mut self, k: K, v: V) {
        self.tick += 1;
        if self.map.len() >= self.cap {
            // clone the candidate key so we don't alias self.map
            if let Some(old_k) = self
                .map
                .iter()
                .map(|(k, (_, t))| (k.clone(), *t))
                .min_by_key(|(_, t)| *t)
                .map(|(k, _)| k)
            {
                self.map.remove(&old_k);
            }
        }
        self.map.insert(k, (v, self.tick));
    }
}
