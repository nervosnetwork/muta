use core_runtime::{DatabaseError, FutRuntimeResult};
use core_storage::{errors::StorageError, storage::Storage};
use core_types::{Block, BlockHeader, Bloom, Hash, Receipt, SignedTransaction};
use futures::future::{err, ok};
use std::collections::hash_map::HashMap;

#[derive(Default, Debug, Clone)]
pub struct MockStorage {
    pub blocks: Vec<Block>,
    pub hashes_height_map: HashMap<Hash, usize>,
    pub transactions: HashMap<Hash, SignedTransaction>,
    pub receipts: HashMap<Hash, Receipt>,
}

impl MockStorage {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let genesis = Block {
            header: BlockHeader {
                prevhash: [0; 32].into(),
                timestamp: 0,
                height: 0,
                transactions_root: [0; 32].into(),
                state_root: [0; 32].into(),
                receipts_root: [0; 32].into(),
                logs_bloom: Bloom::default(),
                quota_used: 0,
                quota_limit: 0,
                votes: vec![],
                proposer: [0; 20].into(),
            },
            tx_hashes: vec![],
        };
        Self {
            blocks: vec![genesis],
            hashes_height_map: HashMap::new(),
            transactions: HashMap::new(),
            receipts: HashMap::new(),
        }
    }
}

impl Storage for MockStorage {
    fn get_latest_block(&self) -> FutRuntimeResult<Block, StorageError> {
        if self.blocks.is_empty() {
            Box::new(err(StorageError::Database(DatabaseError::NotFound)))
        } else {
            Box::new(ok(self.blocks.last().cloned().unwrap()))
        }
    }

    fn get_block_by_height(&self, height: u64) -> FutRuntimeResult<Block, StorageError> {
        if height < self.blocks.len() as u64 {
            Box::new(ok(self.blocks[height as usize].clone()))
        } else {
            Box::new(err(StorageError::Database(DatabaseError::NotFound)))
        }
    }

    fn get_block_by_hash(&self, hash: &Hash) -> FutRuntimeResult<Block, StorageError> {
        let height = self.hashes_height_map.get(hash);
        match height {
            None => Box::new(err(StorageError::Database(DatabaseError::NotFound))),
            Some(height) => Box::new(ok(self.blocks[*height as usize].clone())),
        }
    }

    fn get_transaction(&self, hash: &Hash) -> FutRuntimeResult<SignedTransaction, StorageError> {
        match self.transactions.get(hash) {
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
            .map(|h| self.transactions.get(h).cloned())
            .collect::<Vec<_>>()))
    }

    fn get_receipt(&self, tx_hash: &Hash) -> FutRuntimeResult<Receipt, StorageError> {
        match self.receipts.get(tx_hash) {
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
            .map(|h| self.receipts.get(h).cloned())
            .collect::<Vec<_>>()))
    }

    fn insert_block(&mut self, block: &Block) -> FutRuntimeResult<(), StorageError> {
        if block.header.prevhash
            != self
                .blocks
                .last()
                .map_or(Hash::from_raw(vec![].as_slice()), Block::hash)
        {
            return Box::new(err(StorageError::Internal(
                "prevhash doesn't match".to_string(),
            )));
        }
        if !block
            .tx_hashes
            .iter()
            .all(|h| self.transactions.contains_key(h))
        {
            return Box::new(err(StorageError::Internal(
                "some transaction not exist".to_string(),
            )));
        }
        if !block
            .tx_hashes
            .iter()
            .all(|h| self.receipts.contains_key(h))
        {
            return Box::new(err(StorageError::Internal(
                "some receipts not exist".to_string(),
            )));
        }
        self.hashes_height_map
            .insert(block.hash(), self.blocks.len());
        self.blocks.push(block.clone());
        Box::new(ok(()))
    }

    fn insert_transactions(
        &mut self,
        signed_txs: &[SignedTransaction],
    ) -> FutRuntimeResult<(), StorageError> {
        for tx in signed_txs {
            self.transactions.insert(tx.hash.clone(), tx.clone());
        }
        Box::new(ok(()))
    }

    fn insert_receipts(&mut self, receipts: &[Receipt]) -> FutRuntimeResult<(), StorageError> {
        for receipt in receipts {
            self.receipts
                .insert(receipt.transaction_hash.clone(), receipt.clone());
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
