use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;

use bytes::Bytes;
use derive_more::{Display, From};

use protocol::traits::executor::contract::ContractStateAdapter;
use protocol::traits::executor::{ContractSchema, ContractSer, TrieDB};
use protocol::types::MerkleRoot;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::trie::MPTTrie;

pub type RcGeneralContractStateAdapter<DB> = Rc<RefCell<GeneralContractStateAdapter<DB>>>;

pub struct GeneralContractStateAdapter<DB: TrieDB> {
    trie: MPTTrie<DB>,

    // TODO(@yejiayu): The value of HashMap should be changed to Box<dyn Any> to avoid multiple
    // serializations.
    cache_map: HashMap<Bytes, Bytes>,
    stash_map: HashMap<Bytes, Bytes>,
}

impl<DB: TrieDB> GeneralContractStateAdapter<DB> {
    pub fn new(trie: MPTTrie<DB>) -> Self {
        Self {
            trie,

            cache_map: HashMap::new(),
            stash_map: HashMap::new(),
        }
    }
}

impl<DB: TrieDB> ContractStateAdapter for GeneralContractStateAdapter<DB> {
    fn get<Schema: ContractSchema>(
        &self,
        key: &<Schema as ContractSchema>::Key,
    ) -> ProtocolResult<Option<<Schema as ContractSchema>::Value>> {
        let encoded_key = key.encode()?;

        if let Some(value_bytes) = self.cache_map.get(&encoded_key) {
            let inst = <_>::decode(value_bytes.clone())?;
            return Ok(Some(inst));
        }

        if let Some(value_bytes) = self.stash_map.get(&encoded_key) {
            let inst = <_>::decode(value_bytes.clone())?;
            return Ok(Some(inst));
        }

        if let Some(value_bytes) = self.trie.get(&encoded_key)? {
            return Ok(Some(Schema::Value::decode(value_bytes)?));
        }

        Ok(None)
    }

    fn contains<Schema: ContractSchema>(
        &self,
        key: &<Schema as ContractSchema>::Key,
    ) -> ProtocolResult<bool> {
        let encoded_key = key.encode()?;

        if self.cache_map.contains_key(&encoded_key) {
            return Ok(true);
        };

        if self.stash_map.contains_key(&encoded_key) {
            return Ok(true);
        };

        self.trie.contains(&encoded_key)
    }

    fn insert_cache<Schema: ContractSchema>(
        &mut self,
        key: <Schema as ContractSchema>::Key,
        value: <Schema as ContractSchema>::Value,
    ) -> ProtocolResult<()> {
        self.cache_map.insert(key.encode()?, value.encode()?);
        Ok(())
    }

    fn revert_cache(&mut self) -> ProtocolResult<()> {
        self.cache_map.clear();
        Ok(())
    }

    fn stash(&mut self) -> ProtocolResult<()> {
        for (k, v) in self.cache_map.drain() {
            self.stash_map.insert(k, v);
        }

        Ok(())
    }

    fn commit(&mut self) -> ProtocolResult<MerkleRoot> {
        for (key, value) in self.stash_map.drain() {
            self.trie.insert(key, value)?;
        }

        let root = self.trie.commit()?;
        Ok(root)
    }
}

#[derive(Debug, Display, From)]
pub enum GeneralContractStateAdapterError {
    NotFound { key: String },
}

impl Error for GeneralContractStateAdapterError {}

impl From<GeneralContractStateAdapterError> for ProtocolError {
    fn from(err: GeneralContractStateAdapterError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
