//! A small bounded cache for values that live as long as the isolate.
//!
//! Workers reuse an isolate across requests, so work whose result depends only
//! on deployment configuration (an imported `CryptoKey`, say) can be done once
//! per isolate instead of once per request. Lookup is linear, so keep `cap`
//! small; the oldest entry is evicted first.

use std::borrow::Borrow;

pub struct BoundedCache<K, V> {
    cap: usize,
    entries: Vec<(K, V)>,
}

impl<K: PartialEq, V: Clone> BoundedCache<K, V> {
    pub const fn new(cap: usize) -> Self {
        Self {
            cap,
            entries: Vec::new(),
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.entries
            .iter()
            .find(|(k, _)| k.borrow() == key)
            .map(|(_, v)| v.clone())
    }

    /// Insert or replace `key`. When full, the oldest entry is dropped.
    pub fn insert(&mut self, key: K, value: V) {
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
            return;
        }
        if self.cap == 0 {
            return;
        }
        if self.entries.len() >= self.cap {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
