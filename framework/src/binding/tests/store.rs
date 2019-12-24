use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use bytes::Bytes;
use cita_trie::MemoryDB;

use protocol::traits::{StoreArray, StoreBool, StoreMap, StoreString, StoreUint64};
use protocol::types::Hash;

use crate::binding::store::{
    DefaultStoreArray, DefaultStoreBool, DefaultStoreMap, DefaultStoreString, DefaultStoreUint64,
};
use crate::binding::tests::state::new_state;

#[test]
fn test_default_store_bool() {
    let memdb = Arc::new(MemoryDB::new(false));
    let state = new_state(Arc::clone(&memdb), None);

    let mut sb = DefaultStoreBool::new(Rc::new(RefCell::new(state)), "test");

    sb.set(true).unwrap();
    assert_eq!(sb.get().unwrap(), true);
    sb.set(false).unwrap();
    assert_eq!(sb.get().unwrap(), false);
}

#[test]
fn test_default_store_uint64() {
    let memdb = Arc::new(MemoryDB::new(false));
    let state = new_state(Arc::clone(&memdb), None);

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
    let state = new_state(Arc::clone(&memdb), None);

    let rs = Rc::new(RefCell::new(state));
    let mut ss = DefaultStoreString::new(Rc::clone(&rs), "test");

    ss.set("").unwrap();
    assert_eq!(ss.get().unwrap(), "");
    assert_eq!(ss.is_empty().unwrap(), true);

    ss.set("ok").unwrap();
    assert_eq!(ss.get().unwrap(), String::from("ok"));
    assert_eq!(ss.len().unwrap(), 2usize);
}

#[test]
fn test_default_store_map() {
    let memdb = Arc::new(MemoryDB::new(false));
    let state = new_state(Arc::clone(&memdb), None);
    let rs = Rc::new(RefCell::new(state));

    let mut sm = DefaultStoreMap::<_, Hash, Bytes>::new(Rc::clone(&rs), "test");

    sm.insert(Hash::digest(Bytes::from("key_1")), Bytes::from("val_1"))
        .unwrap();
    sm.insert(Hash::digest(Bytes::from("key_2")), Bytes::from("val_2"))
        .unwrap();

    assert_eq!(
        sm.get(&Hash::digest(Bytes::from("key_1"))).unwrap(),
        Bytes::from("val_1")
    );
    assert_eq!(
        sm.get(&Hash::digest(Bytes::from("key_2"))).unwrap(),
        Bytes::from("val_2")
    );

    sm.remove(&Hash::digest(Bytes::from("key_1"))).unwrap();

    assert_eq!(
        sm.contains(&Hash::digest(Bytes::from("key_1"))).unwrap(),
        false
    );
    assert_eq!(sm.len().unwrap(), 1usize)
}

#[test]
fn test_default_store_array() {
    let memdb = Arc::new(MemoryDB::new(false));
    let state = new_state(Arc::clone(&memdb), None);
    let rs = Rc::new(RefCell::new(state));

    let mut sa = DefaultStoreArray::<_, Bytes>::new(Rc::clone(&rs), "test");

    assert_eq!(sa.len().unwrap(), 0usize);

    sa.push(Bytes::from("111")).unwrap();
    sa.push(Bytes::from("222")).unwrap();

    assert_eq!(sa.get(0usize).unwrap(), Bytes::from("111"));
    assert_eq!(sa.get(1usize).unwrap(), Bytes::from("222"));

    sa.remove(0usize).unwrap();

    assert_eq!(sa.len().unwrap(), 1usize);
    assert_eq!(sa.get(0usize).unwrap(), Bytes::from("222"));
}
