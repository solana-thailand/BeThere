//! `.plans/028` W12 — the per-isolate cache behind the HMAC `CryptoKey` reuse.

use event_checkin_worker::isolate_cache::BoundedCache;

#[test]
fn returns_what_was_inserted_and_nothing_else() {
    let mut cache = BoundedCache::new(2);
    assert!(cache.is_empty());
    cache.insert([1u8; 32], "a");
    assert_eq!(cache.get(&[1u8; 32]), Some("a"));
    assert_eq!(cache.get(&[2u8; 32]), None);
}

#[test]
fn evicts_the_oldest_entry_at_capacity() {
    let mut cache = BoundedCache::new(2);
    cache.insert(1, "a");
    cache.insert(2, "b");
    cache.insert(3, "c");
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&1), None);
    assert_eq!(cache.get(&2), Some("b"));
    assert_eq!(cache.get(&3), Some("c"));
}

#[test]
fn reinserting_a_key_replaces_without_growing() {
    let mut cache = BoundedCache::new(2);
    cache.insert(1, "a");
    cache.insert(1, "b");
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get(&1), Some("b"));
}

#[test]
fn zero_capacity_caches_nothing() {
    let mut cache = BoundedCache::new(0);
    cache.insert(1, "a");
    assert_eq!(cache.get(&1), None);
}
