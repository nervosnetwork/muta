use std::sync::Arc;
use std::cell::RefCell;
use std::rc::Rc;

use cita_trie::MemoryDB;

use protocol::traits::{StoreBool, StoreString, StoreUint64};
use protocol::types::MerkleRoot;

use crate::state::{GeneralServiceState, MPTTrie};
use crate::store::{DefaultStoreBool, DefaultStoreString, DefaultStoreUint64};

#[test]
fn test_default_store_bool() {
    let memdb = Arc::new(MemoryDB::new(false));
    let mut state = new_state(Arc::clone(&memdb), None);

    let mut sb = DefaultStoreBool::new(Rc::new(RefCell::new(state)), "test");

    sb.set(true).unwrap();
    assert_eq!(sb.get().unwrap(), true);
    sb.set(false).unwrap();
    assert_eq!(sb.get().unwrap(), false);
}

#[test]
fn test_default_store_uint64() {
    let memdb = Arc::new(MemoryDB::new(false));
    let mut state = new_state(Arc::clone(&memdb), None);

    let mut su = DefaultStoreUint64::new(Rc::new(RefCell::new(state)), "test");

    su.set(8u64).unwrap();
    assert_eq!(su.get().unwrap(), 8u64);

    su.add(12u64).unwrap();
    assert_eq!(su.get().unwrap(), 20u64);

    su.sub(10u64).unwrap();
    assert_eq!(su.get().unwrap(), 10u64);

    su.mul(8u64).unwrap();
    assert_eq!(su.get().unwrap(), 80u64);

    su.div(10u64).unwrap();
    assert_eq!(su.get().unwrap(), 8u64);

    su.pow(2u32).unwrap();
    assert_eq!(su.get().unwrap(), 64u64);

    su.rem(5u64).unwrap();
    assert_eq!(su.get().unwrap(), 4u64);
}

#[test]
fn test_default_store_string() {
    let memdb = Arc::new(MemoryDB::new(false));
    let mut state = new_state(Arc::clone(&memdb), None);

    let mut ss = DefaultStoreString::new(Rc::new(RefCell::new(state)), "test");

    ss.set("").unwrap();
    assert_eq!(ss.get().unwrap(), "");
    assert_eq!(ss.is_empty().unwrap(), true);

    ss.set("ok").unwrap();
    assert_eq!(ss.get().unwrap(), String::from("ok"));
    assert_eq!(ss.len().unwrap(), 2usize);
}

fn new_state(memdb: Arc<MemoryDB>, root: Option<MerkleRoot>) -> GeneralServiceState<MemoryDB> {
    let trie = match root {
        Some(root) => MPTTrie::from(root, memdb).unwrap(),
        None => MPTTrie::new(memdb),
    };

    GeneralServiceState::new(trie)
}
