use std::sync::Arc;

use bytes::Bytes;
use cita_trie::MemoryDB;

use protocol::traits::ServiceState;
use protocol::types::{Address, Hash, MerkleRoot};

use crate::binding::state::{GeneralServiceState, MPTTrie};

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
