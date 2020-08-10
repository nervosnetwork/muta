extern crate test;

use std::collections::HashSet;
use std::sync::Arc;

use bytes::Bytes;
use cita_trie::{MemoryDB, DB};
use test::Bencher;

use protocol::traits::ServiceState;
use protocol::types::{Address, Hash, MerkleRoot};

use crate::binding::state::{GeneralServiceState, MPTTrie, RocksTrieDB};

#[rustfmt::skip]
/// Bench in AMD Ryzen 7 3800X 8-Core Processor (16 x 4250)
/// test binding::tests::state::bench_get_cache_hit              ... bench:          47 ns/iter (+/- 3)
/// test binding::tests::state::bench_get_cache_miss             ... bench:       1,063 ns/iter (+/- 35)
/// test binding::tests::state::bench_get_without_cache          ... bench:         526 ns/iter (+/- 19)
/// test binding::tests::state::bench_insert_batch_with_cache    ... bench:   1,113,015 ns/iter (+/- 489,068)
/// test binding::tests::state::bench_insert_batch_without_cache ... bench:     979,408 ns/iter (+/- 510,953)
/// test binding::tests::state::bench_insert_with_cache          ... bench:       2,716 ns/iter (+/- 602)
/// test binding::tests::state::bench_insert_without_cache       ... bench:       2,491 ns/iter (+/- 486)
#[bench]
fn bench_insert_batch_with_cache(b: &mut Bencher) {
    let triedb = new_triedb();

    let keys = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();
    let values = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();

    b.iter(|| {
        triedb.insert_batch(keys.clone(), values.clone()).unwrap();
    })
}

#[bench]
fn bench_insert_batch_without_cache(b: &mut Bencher) {
    let triedb = new_triedb();

    let keys = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();
    let values = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();

    b.iter(|| {
        triedb.insert_batch_without_cache(keys.clone(), values.clone());
    })
}

#[bench]
fn bench_insert_with_cache(b: &mut Bencher) {
    let triedb = new_triedb();

    let key = rand_bytes();
    let value = rand_bytes();

    b.iter(|| {
        triedb.insert(key.clone(), value.clone()).unwrap();
    })
}

#[bench]
fn bench_insert_without_cache(b: &mut Bencher) {
    let triedb = new_triedb();

    let key = rand_bytes();
    let value = rand_bytes();

    b.iter(|| {
        triedb.insert_without_cache(key.clone(), value.clone());
    })
}

#[bench]
fn bench_get_cache_hit(b: &mut Bencher) {
    let triedb = new_triedb();

    let keys = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();
    let values = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();

    triedb.insert_batch(keys.clone(), values).unwrap();

    let key = keys[0].clone();
    b.iter(|| {
        let _ = triedb.get(&key).unwrap();
    })
}

#[bench]
fn bench_get_cache_miss(b: &mut Bencher) {
    let triedb = new_triedb();

    let keys = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();
    let values = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();

    triedb.insert_batch(keys.clone(), values).unwrap();

    let keys = keys.iter().collect::<HashSet<_>>();
    let key = {
        let mut tmp = rand_bytes();
        while keys.contains(&tmp) {
            tmp = rand_bytes();
        }
        tmp
    };

    b.iter(|| {
        let _ = triedb.get(&key).unwrap();
    })
}

#[bench]
fn bench_get_without_cache(b: &mut Bencher) {
    let triedb = new_triedb();

    let keys = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();
    let values = (0..1000).map(|_| rand_bytes()).collect::<Vec<_>>();

    triedb.insert_batch_without_cache(keys.clone(), values);

    let key = keys[0].clone();
    b.iter(|| {
        let _ = triedb.get_without_cache(&key).unwrap();
    })
}

#[test]
fn test_trie_db() {
    let triedb = new_triedb();
    let key = rand_bytes();
    let value = rand_bytes();

    triedb.insert(key.clone(), value.clone()).unwrap();
    assert_eq!(triedb.get(&key).unwrap().unwrap(), value);

    let keys = (0..3000).map(|_| rand_bytes()).collect::<Vec<_>>();
    let values = (0..3000).map(|_| rand_bytes()).collect::<Vec<_>>();
    triedb.insert_batch(keys.clone(), values.clone()).unwrap();

    let _ = keys
        .iter()
        .zip(values.iter())
        .map(|(k, v)| assert_eq!(&triedb.get(k).unwrap().unwrap(), v));
    assert_eq!(triedb.cache().len(), 3001);

    triedb.flush().unwrap();
    assert_eq!(triedb.cache().len(), 2000);

    let _ = keys.iter().map(|k| assert!(triedb.contains(k).unwrap()));
}

#[test]
fn test_state_insert() {
    let memdb = Arc::new(MemoryDB::new(false));
    let mut state = new_state(Arc::clone(&memdb), None);

    let key = Hash::digest(Bytes::from("key".to_owned()));
    let value = Hash::digest(Bytes::from("value".to_owned()));
    state.insert(key.clone(), value.clone()).unwrap();
    let val: Hash = state.get(&key).unwrap().unwrap();
    assert_eq!(val, value);

    state.stash().unwrap();
    let new_root = state.commit().unwrap();

    let val: Hash = state.get(&key).unwrap().unwrap();
    assert_eq!(val, value);

    let new_state = new_state(Arc::clone(&memdb), Some(new_root));
    let val: Hash = new_state.get(&key).unwrap().unwrap();
    assert_eq!(val, value);
}

#[test]
fn test_state_account() {
    let memdb = Arc::new(MemoryDB::new(false));
    let mut state = new_state(Arc::clone(&memdb), None);

    let address = Address::from_hash(Hash::digest(Bytes::from("test-address"))).unwrap();
    let key = Hash::digest(Bytes::from("key".to_owned()));
    let value = Hash::digest(Bytes::from("value".to_owned()));

    state
        .set_account_value(&address, key.clone(), value.clone())
        .unwrap();
    let val: Hash = state.get_account_value(&address, &key).unwrap().unwrap();
    assert_eq!(val, value);

    state.stash().unwrap();
    let new_root = state.commit().unwrap();

    let new_state = new_state(Arc::clone(&memdb), Some(new_root));
    let val: Hash = new_state
        .get_account_value(&address, &key)
        .unwrap()
        .unwrap();
    assert_eq!(val, value);
}

pub fn new_state(memdb: Arc<MemoryDB>, root: Option<MerkleRoot>) -> GeneralServiceState<MemoryDB> {
    let trie = match root {
        Some(root) => MPTTrie::from(root, memdb).unwrap(),
        None => MPTTrie::new(memdb),
    };

    GeneralServiceState::new(trie)
}

fn new_triedb() -> RocksTrieDB {
    RocksTrieDB::new("./free-space", false, 1024, 2000).unwrap()
}

fn rand_bytes() -> Vec<u8> {
    (0..32).map(|_| rand::random::<u8>()).collect::<Vec<u8>>()
}
