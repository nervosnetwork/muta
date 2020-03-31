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

    assert_eq!(sb.get(), false);
    sb.set(true);
    assert_eq!(sb.get(), true);
    sb.set(false);
    assert_eq!(sb.get(), false);
}

#[test]
fn test_default_store_uint64() {
    let memdb = Arc::new(MemoryDB::new(false));
    let state = new_state(Arc::clone(&memdb), None);

    let mut su = DefaultStoreUint64::new(Rc::new(RefCell::new(state)), "test");

    assert_eq!(su.get(), 0u64);
    su.set(8u64);
    assert_eq!(su.get(), 8u64);

    su.add(12u64);
    assert_eq!(su.get(), 20u64);

    su.sub(10u64);
    assert_eq!(su.get(), 10u64);

    su.mul(8u64);
    assert_eq!(su.get(), 80u64);

    su.div(10u64);
    assert_eq!(su.get(), 8u64);

    su.pow(2u32);
    assert_eq!(su.get(), 64u64);

    su.rem(5u64);
    assert_eq!(su.get(), 4u64);
}

#[test]
fn test_default_store_string() {
    let memdb = Arc::new(MemoryDB::new(false));
    let state = new_state(Arc::clone(&memdb), None);

    let rs = Rc::new(RefCell::new(state));
    let mut ss = DefaultStoreString::new(Rc::clone(&rs), "test");

    assert_eq!(ss.get(), "");

    ss.set("");
    assert_eq!(ss.get(), "");
    assert_eq!(ss.is_empty(), true);

    ss.set("ok");
    assert_eq!(ss.get(), String::from("ok"));
    assert_eq!(ss.len(), 2u32);
}

#[test]
fn test_default_store_map() {
    let memdb = Arc::new(MemoryDB::new(false));
    let state = new_state(Arc::clone(&memdb), None);
    let rs = Rc::new(RefCell::new(state));

    let mut sm = DefaultStoreMap::<_, Hash, Bytes>::new(Rc::clone(&rs), "test");

    assert_eq!(sm.get(&Hash::digest(Bytes::from("key_1"))).is_none(), true);
    sm.insert(Hash::digest(Bytes::from("key_1")), Bytes::from("val_1"));
    sm.insert(Hash::digest(Bytes::from("key_2")), Bytes::from("val_2"));

    {
        let mut it = sm.iter();
        assert_eq!(
            it.next().unwrap(),
            (&Hash::digest(Bytes::from("key_1")), Bytes::from("val_1"))
        );
        assert_eq!(
            it.next().unwrap(),
            (&Hash::digest(Bytes::from("key_2")), Bytes::from("val_2"))
        );
        assert_eq!(it.next().is_none(), true);
    }

    assert_eq!(
        sm.get(&Hash::digest(Bytes::from("key_1"))).unwrap(),
        Bytes::from("val_1")
    );
    assert_eq!(
        sm.get(&Hash::digest(Bytes::from("key_2"))).unwrap(),
        Bytes::from("val_2")
    );

    sm.remove(&Hash::digest(Bytes::from("key_1"))).unwrap();

    assert_eq!(sm.contains(&Hash::digest(Bytes::from("key_1"))), false);
    assert_eq!(sm.len(), 1u32)
}

#[test]
fn test_default_store_array() {
    let memdb = Arc::new(MemoryDB::new(false));
    let state = new_state(Arc::clone(&memdb), None);
    let rs = Rc::new(RefCell::new(state));

    let mut sa = DefaultStoreArray::<_, Bytes>::new(Rc::clone(&rs), "test");

    assert_eq!(sa.len(), 0u32);
    assert_eq!(sa.get(0u32).is_none(), true);

    sa.push(Bytes::from("111"));
    sa.push(Bytes::from("222"));

    assert_eq!(sa.get(3u32).is_none(), true);

    {
        let mut it = sa.iter();
        assert_eq!(it.next().unwrap(), (0u32, Bytes::from("111")));
        assert_eq!(it.next().unwrap(), (1u32, Bytes::from("222")));
        assert_eq!(it.next().is_none(), true);
    }

    assert_eq!(sa.get(0u32).unwrap(), Bytes::from("111"));
    assert_eq!(sa.get(1u32).unwrap(), Bytes::from("222"));

    sa.remove(0u32);

    assert_eq!(sa.len(), 1u32);
    assert_eq!(sa.get(0u32).unwrap(), Bytes::from("222"));
}
