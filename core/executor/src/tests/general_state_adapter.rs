use std::sync::Arc;

use bytes::Bytes;

use protocol::traits::executor::contract::ContractStateAdapter;
use protocol::traits::executor::{ContractSchema, ContractSer};
use protocol::ProtocolResult;

use crate::adapter::GeneralContractStateAdapter;
use crate::tests;

struct FixedTestSchema;
impl ContractSchema for FixedTestSchema {
    type Key = FixedTestBytes;
    type Value = FixedTestBytes;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedTestBytes {
    inner: Bytes,
}

impl FixedTestBytes {
    pub fn new(inner: Bytes) -> Self {
        Self { inner }
    }
}

impl ContractSer for FixedTestBytes {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(self.inner.clone())
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(FixedTestBytes { inner: bytes })
    }
}

#[test]
fn insert() {
    let memdb = tests::create_empty_memdb();
    let trie = tests::create_empty_trie(Arc::clone(&memdb));
    let mut state_adapter = GeneralContractStateAdapter::new(trie);

    let key = FixedTestBytes::new(Bytes::from(b"test-key".to_vec()));
    let value = FixedTestBytes::new(Bytes::from(b"test-value".to_vec()));
    state_adapter
        .insert_cache::<FixedTestSchema>(key.clone(), value.clone())
        .unwrap();

    let get_value = state_adapter.get::<FixedTestSchema>(&key).unwrap().unwrap();
    assert_eq!(get_value, value.clone());
    state_adapter.stash().unwrap();

    let get_value = state_adapter.get::<FixedTestSchema>(&key).unwrap().unwrap();
    assert_eq!(get_value, value.clone());

    state_adapter.commit().unwrap();
    let get_value = state_adapter.get::<FixedTestSchema>(&key).unwrap().unwrap();
    assert_eq!(get_value, value.clone());
}

#[test]
fn revert() {
    let memdb = tests::create_empty_memdb();
    let trie = tests::create_empty_trie(Arc::clone(&memdb));
    let mut state_adapter = GeneralContractStateAdapter::new(trie);

    let key = FixedTestBytes::new(Bytes::from(b"test-key".to_vec()));
    let value = FixedTestBytes::new(Bytes::from(b"test-value".to_vec()));
    state_adapter
        .insert_cache::<FixedTestSchema>(key.clone(), value.clone())
        .unwrap();

    let get_value = state_adapter.get::<FixedTestSchema>(&key).unwrap().unwrap();
    assert_eq!(get_value, value.clone());

    state_adapter.revert_cache().unwrap();
    let get_value = state_adapter.get::<FixedTestSchema>(&key).unwrap();
    assert_eq!(get_value, None);
}
