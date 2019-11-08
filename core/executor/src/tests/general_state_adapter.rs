use std::sync::Arc;

use bytes::Bytes;

use protocol::traits::executor::contract::ContractStateAdapter;

use crate::adapter::GeneralContractStateAdapter;
use crate::fixed_types::FixedBytesSchema;
use crate::tests;

#[test]
fn insert() {
    let memdb = tests::create_empty_memdb();
    let trie = tests::create_empty_trie(Arc::clone(&memdb));
    let mut state_adapter = GeneralContractStateAdapter::new(trie);

    let key = Bytes::from(b"test-key".to_vec());
    let value = Bytes::from(b"test-value".to_vec());
    state_adapter
        .insert_cache::<FixedBytesSchema>(key.clone(), value.clone())
        .unwrap();

    let get_value = state_adapter
        .get::<FixedBytesSchema>(&key)
        .unwrap()
        .unwrap();
    assert_eq!(get_value, value.clone());
    state_adapter.stash().unwrap();

    let get_value = state_adapter
        .get::<FixedBytesSchema>(&key)
        .unwrap()
        .unwrap();
    assert_eq!(get_value, value.clone());

    state_adapter.commit().unwrap();
    let get_value = state_adapter
        .get::<FixedBytesSchema>(&key)
        .unwrap()
        .unwrap();
    assert_eq!(get_value, value);
}

#[test]
fn revert() {
    let memdb = tests::create_empty_memdb();
    let trie = tests::create_empty_trie(Arc::clone(&memdb));
    let mut state_adapter = GeneralContractStateAdapter::new(trie);

    let key = Bytes::from(b"test-key".to_vec());
    let value = Bytes::from(b"test-value".to_vec());
    state_adapter
        .insert_cache::<FixedBytesSchema>(key.clone(), value.clone())
        .unwrap();

    let get_value = state_adapter
        .get::<FixedBytesSchema>(&key)
        .unwrap()
        .unwrap();
    assert_eq!(get_value, value);

    state_adapter.revert_cache().unwrap();
    let get_value = state_adapter.get::<FixedBytesSchema>(&key).unwrap();
    assert_eq!(get_value, None);
}
