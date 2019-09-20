use std::error::Error;
use std::sync::Arc;

use bytes::Bytes;
use cita_trie::{PatriciaTrie, Trie, TrieError};
use derive_more::{Display, From};
use hasher::HasherKeccak;
use lazy_static::lazy_static;

use protocol::types::{Hash, MerkleRoot};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

lazy_static! {
    static ref HASHER_INST: Arc<HasherKeccak> = Arc::new(HasherKeccak::new());
}

pub struct MPTTrie {
    root: MerkleRoot,
    trie: PatriciaTrie<cita_trie::MemoryDB, HasherKeccak>,
}

impl MPTTrie {
    pub fn new(db: Arc<cita_trie::MemoryDB>) -> Self {
        let trie = PatriciaTrie::new(db, Arc::clone(&HASHER_INST));

        Self {
            root: Hash::from_empty(),
            trie,
        }
    }

    pub fn from(root: MerkleRoot, db: Arc<cita_trie::MemoryDB>) -> ProtocolResult<Self> {
        let trie = PatriciaTrie::from(db, Arc::clone(&HASHER_INST), &root.as_bytes())
            .map_err(MPTTrieError::from)?;

        Ok(Self { root, trie })
    }

    pub fn get(&self, key: Bytes) -> ProtocolResult<Option<Bytes>> {
        Ok(self
            .trie
            .get(&key)
            .map_err(MPTTrieError::from)?
            .map(Bytes::from))
    }

    pub fn contains(&self, key: Bytes) -> ProtocolResult<bool> {
        Ok(self.trie.contains(&key).map_err(MPTTrieError::from)?)
    }

    pub fn insert(&mut self, key: Bytes, value: Bytes) -> ProtocolResult<()> {
        self.trie
            .insert(key.to_vec(), value.to_vec())
            .map_err(MPTTrieError::from)?;
        Ok(())
    }

    pub fn commit(&mut self) -> ProtocolResult<MerkleRoot> {
        let root_bytes = self.trie.root().map_err(MPTTrieError::from)?;
        let root = MerkleRoot::from_bytes(Bytes::from(root_bytes))?;
        self.root = root;
        Ok(self.root.clone())
    }
}

#[derive(Debug, Display, From)]
pub enum MPTTrieError {
    #[display(fmt = "{:?}", _0)]
    Trie(TrieError),
}

impl Error for MPTTrieError {}

impl From<MPTTrieError> for ProtocolError {
    fn from(err: MPTTrieError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
