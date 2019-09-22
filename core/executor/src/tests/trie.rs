use std::sync::Arc;

use bytes::Bytes;

use crate::tests;

#[test]
fn insert() {
    let memdb = tests::create_empty_memdb();
    let mut trie = tests::create_empty_trie(Arc::clone(&memdb));

    let key = Bytes::from(b"test-key".to_vec());
    let value = Bytes::from(b"test-value".to_vec());

    trie.insert(key.clone(), value.clone()).unwrap();
    let root = trie.commit().unwrap();
    let get_value = trie.get(&key).unwrap().unwrap();
    assert_eq!(value, get_value);

    let new_trie = tests::create_trie_from_root(root, Arc::clone(&memdb));
    let get_value = new_trie.get(&key).unwrap().unwrap();
    assert_eq!(value, get_value)
}

#[test]
fn contains() {
    let memdb = tests::create_empty_memdb();
    let mut trie = tests::create_empty_trie(Arc::clone(&memdb));

    let key = Bytes::from(b"test-key".to_vec());
    let value = Bytes::from(b"test-value".to_vec());

    trie.insert(key.clone(), value.clone()).unwrap();
    assert_eq!(trie.contains(&key).unwrap(), true);
    let root = trie.commit().unwrap();

    let new_trie = tests::create_trie_from_root(root, Arc::clone(&memdb));
    assert_eq!(new_trie.contains(&key).unwrap(), true)
}

#[test]
fn commit() {
    let memdb = tests::create_empty_memdb();
    let mut trie = tests::create_empty_trie(Arc::clone(&memdb));

    let key = Bytes::from(b"test-key".to_vec());
    let value = Bytes::from(b"test-value".to_vec());

    trie.insert(key.clone(), value.clone()).unwrap();
    let root = trie.commit().unwrap();

    let mut new_trie = tests::create_trie_from_root(root.clone(), Arc::clone(&memdb));
    let root2 = new_trie.commit().unwrap();
    assert_eq!(root.clone(), root2.clone());

    let key2 = Bytes::from(b"test-key2".to_vec());
    let value2 = Bytes::from(b"test-value2".to_vec());
    new_trie.insert(key2, value2).unwrap();
    let root3 = new_trie.commit().unwrap();

    assert_eq!(root2 != root3, true)
}
