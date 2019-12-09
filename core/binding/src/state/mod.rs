mod trie;
mod trie_db;

pub use trie::{MPTTrie, MPTTrieError};
pub use trie_db::{RocksTrieDB, RocksTrieDBError};

use std::collections::HashMap;

use bytes::Bytes;
use cita_trie::DB as TrieDB;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::ServiceState;
use protocol::types::{Address, Hash, MerkleRoot};
use protocol::ProtocolResult;

pub struct GeneralServiceState<DB: TrieDB> {
    trie: MPTTrie<DB>,

    // TODO(@yejiayu): The value of HashMap should be changed to Box<dyn Any> to avoid multiple
    // serializations.
    cache_map: HashMap<Bytes, Bytes>,
    stash_map: HashMap<Bytes, Bytes>,
}

impl<DB: TrieDB> GeneralServiceState<DB> {
    pub fn new(trie: MPTTrie<DB>) -> Self {
        Self {
            trie,

            cache_map: HashMap::new(),
            stash_map: HashMap::new(),
        }
    }
}

impl<DB: TrieDB> ServiceState for GeneralServiceState<DB> {
    fn get<Key: FixedCodec, Ret: FixedCodec>(&self, key: &Key) -> ProtocolResult<Option<Ret>> {
        let encoded_key = key.encode_fixed()?;

        if let Some(value_bytes) = self.cache_map.get(&encoded_key) {
            let inst = <_>::decode_fixed(value_bytes.clone())?;
            return Ok(Some(inst));
        }

        if let Some(value_bytes) = self.stash_map.get(&encoded_key) {
            let inst = <_>::decode_fixed(value_bytes.clone())?;
            return Ok(Some(inst));
        }

        if let Some(value_bytes) = self.trie.get(&encoded_key)? {
            return Ok(Some(<_>::decode_fixed(value_bytes)?));
        }

        Ok(None)
    }

    fn contains<Key: FixedCodec>(&self, key: &Key) -> ProtocolResult<bool> {
        let encoded_key = key.encode_fixed()?;

        if self.cache_map.contains_key(&encoded_key) {
            return Ok(true);
        };

        if self.stash_map.contains_key(&encoded_key) {
            return Ok(true);
        };

        self.trie.contains(&encoded_key)
    }

    // Insert a pair of key / value
    // Note: This key/value pair will go into the cache first
    // and will not be persisted to MPT until `commit` is called.
    fn insert<Key: FixedCodec, Value: FixedCodec>(
        &mut self,
        key: Key,
        value: Value,
    ) -> ProtocolResult<()> {
        self.cache_map
            .insert(key.encode_fixed()?, value.encode_fixed()?);
        Ok(())
    }

    fn get_account_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        address: &Address,
        key: &Key,
    ) -> ProtocolResult<Option<Ret>> {
        let hash_key = get_address_key(address, key)?;
        self.get(&hash_key)
    }

    fn set_account_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        address: &Address,
        key: Key,
        val: Val,
    ) -> ProtocolResult<()> {
        let hash_key = get_address_key(address, &key)?;
        self.insert(hash_key, val)
    }

    // Roll back all data in the cache
    fn revert_cache(&mut self) -> ProtocolResult<()> {
        self.cache_map.clear();
        Ok(())
    }

    // Move data from cache to stash
    fn stash(&mut self) -> ProtocolResult<()> {
        for (k, v) in self.cache_map.drain() {
            self.stash_map.insert(k, v);
        }

        Ok(())
    }

    // Persist data from stash into MPT
    fn commit(&mut self) -> ProtocolResult<MerkleRoot> {
        for (key, value) in self.stash_map.drain() {
            self.trie.insert(key, value)?;
        }

        let root = self.trie.commit()?;
        Ok(root)
    }
}

fn get_address_key<Key: FixedCodec>(address: &Address, key: &Key) -> ProtocolResult<Hash> {
    let mut hash_bytes = address.as_bytes().to_vec();
    hash_bytes.extend_from_slice(key.encode_fixed()?.as_ref());

    Ok(Hash::digest(Bytes::from(hash_bytes)))
}
