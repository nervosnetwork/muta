use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use futures::future::{err, ok};

use core_runtime::{DatabaseError, FutRuntimeResult};
use core_storage::{errors::StorageError, storage::Storage};
use core_types::{
    Address, Block, BlockHeader, Bloom, Hash, Receipt, SignedTransaction, TransactionPosition,
};

#[derive(Default, Debug, Clone)]
pub struct MockStorage {
    pub blocks: Arc<RwLock<Vec<Block>>>,
    pub hashes_height_map: Arc<RwLock<HashMap<Hash, usize>>>,
    pub transactions: Arc<RwLock<HashMap<Hash, SignedTransaction>>>,
    pub receipts: Arc<RwLock<HashMap<Hash, Receipt>>>,
    pub transaction_positions: Arc<RwLock<HashMap<Hash, TransactionPosition>>>,
}

impl MockStorage {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let genesis = Block {
            header: BlockHeader {
                prevhash: Hash::from_fixed_bytes([0; 32]),
                timestamp: 0,
                height: 0,
                transactions_root: Hash::from_fixed_bytes([0; 32]),
                state_root: Hash::from_fixed_bytes([0; 32]),
                receipts_root: Hash::from_fixed_bytes([0; 32]),
                logs_bloom: Bloom::default(),
                quota_used: 0,
                quota_limit: 0,
                votes: vec![],
                proposer: Address::from_fixed_bytes([0; 20]),
            },
            tx_hashes: vec![],
        };
        Self {
            blocks: Arc::new(RwLock::new(vec![genesis])),
            hashes_height_map: Arc::new(RwLock::new(HashMap::new())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            receipts: Arc::new(RwLock::new(HashMap::new())),
            transaction_positions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Storage for MockStorage {
    fn get_latest_block(&self) -> FutRuntimeResult<Block, StorageError> {
        if self.blocks.read().unwrap().is_empty() {
            Box::new(err(StorageError::Database(DatabaseError::NotFound)))
        } else {
            Box::new(ok(self.blocks.read().unwrap().last().cloned().unwrap()))
        }
    }

    fn get_block_by_height(&self, height: u64) -> FutRuntimeResult<Block, StorageError> {
        if height < self.blocks.read().unwrap().len() as u64 {
            Box::new(ok(self.blocks.read().unwrap()[height as usize].clone()))
        } else {
            Box::new(err(StorageError::Database(DatabaseError::NotFound)))
        }
    }

    fn get_block_by_hash(&self, hash: &Hash) -> FutRuntimeResult<Block, StorageError> {
        let hashes_height_map = self.hashes_height_map.read().unwrap();
        let height = hashes_height_map.get(hash);
        match height {
            None => Box::new(err(StorageError::Database(DatabaseError::NotFound))),
            Some(height) => Box::new(ok(self.blocks.read().unwrap()[*height as usize].clone())),
        }
    }

    fn get_transaction(&self, hash: &Hash) -> FutRuntimeResult<SignedTransaction, StorageError> {
        match self.transactions.read().unwrap().get(hash) {
            None => Box::new(err(StorageError::Database(DatabaseError::NotFound))),
            Some(tx) => Box::new(ok(tx.clone())),
        }
    }

    fn get_transactions(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<SignedTransaction>>, StorageError> {
        Box::new(ok(hashes
            .iter()
            .map(|h| self.transactions.read().unwrap().get(h).cloned())
            .collect::<Vec<_>>()))
    }

    fn get_receipt(&self, tx_hash: &Hash) -> FutRuntimeResult<Receipt, StorageError> {
        match self.receipts.read().unwrap().get(tx_hash) {
            None => Box::new(err(StorageError::Database(DatabaseError::NotFound))),
            Some(tx) => Box::new(ok(tx.clone())),
        }
    }

    fn get_receipts(
        &self,
        tx_hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<Receipt>>, StorageError> {
        Box::new(ok(tx_hashes
            .iter()
            .map(|h| self.receipts.read().unwrap().get(h).cloned())
            .collect::<Vec<_>>()))
    }

    fn get_transaction_position(
        &self,
        hash: &Hash,
    ) -> FutRuntimeResult<TransactionPosition, StorageError> {
        match self.transaction_positions.read().unwrap().get(hash) {
            None => Box::new(err(StorageError::Database(DatabaseError::NotFound))),
            Some(tx) => Box::new(ok(tx.clone())),
        }
    }

    fn get_transaction_positions(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<TransactionPosition>>, StorageError> {
        Box::new(ok(hashes
            .iter()
            .map(|h| self.transaction_positions.read().unwrap().get(h).cloned())
            .collect::<Vec<_>>()))
    }

    fn insert_block(&self, block: Block) -> FutRuntimeResult<(), StorageError> {
        if block.header.prevhash
            != self
                .blocks
                .read()
                .unwrap()
                .last()
                .map_or(Hash::digest(b"test"), |b| b.header.hash())
        {
            return Box::new(err(StorageError::Internal(
                "prevhash doesn't match".to_string(),
            )));
        }
        if !block
            .tx_hashes
            .iter()
            .all(|h| self.transactions.read().unwrap().contains_key(h))
        {
            return Box::new(err(StorageError::Internal(
                "some transaction not exist".to_string(),
            )));
        }
        if !block
            .tx_hashes
            .iter()
            .all(|h| self.receipts.read().unwrap().contains_key(h))
        {
            return Box::new(err(StorageError::Internal(
                "some receipts not exist".to_string(),
            )));
        }
        self.hashes_height_map
            .write()
            .unwrap()
            .insert(block.header.hash(), self.blocks.read().unwrap().len());
        self.blocks.write().unwrap().push(block);
        Box::new(ok(()))
    }

    fn insert_transactions(
        &self,
        signed_txs: Vec<SignedTransaction>,
    ) -> FutRuntimeResult<(), StorageError> {
        for tx in signed_txs {
            let hash = tx.hash.clone();
            self.transactions.write().unwrap().insert(hash, tx);
        }
        Box::new(ok(()))
    }

    fn insert_transaction_positions(
        &self,
        positions: HashMap<Hash, TransactionPosition>,
    ) -> FutRuntimeResult<(), StorageError> {
        for (block_hash, position) in positions {
            self.transaction_positions
                .write()
                .unwrap()
                .insert(block_hash, position);
        }
        Box::new(ok(()))
    }

    fn insert_receipts(&self, receipts: Vec<Receipt>) -> FutRuntimeResult<(), StorageError> {
        for receipt in receipts {
            self.receipts
                .write()
                .unwrap()
                .insert(receipt.transaction_hash.clone(), receipt);
        }
        Box::new(ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::Future;

    #[test]
    fn test_mock_storage() {
        let ms = MockStorage::new();
        assert_eq!(ms.get_latest_block().wait().unwrap().header.height, 0);
    }

}
