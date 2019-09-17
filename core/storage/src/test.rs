#[cfg(test)]
mod tests {
    extern crate test;

    use std::sync::Arc;

    use bytes::Bytes;
    use futures::executor;
    use num_traits::FromPrimitive;
    use rand::random;

    use protocol::codec::ProtocolCodec;
    use protocol::traits::{Storage, StorageAdapter, StorageBatchModify};
    use protocol::types::{
        AccountAddress, Epoch, EpochHeader, Fee, Hash, Proof, RawTransaction, Receipt,
        ReceiptResult, SignedTransaction, TransactionAction,
    };

    use crate::adapter::memory::MemoryAdapter;
    use crate::adapter::rocks::RocksAdapter;
    use crate::{ImplStorage, TransactionSchema};

    macro_rules! exec {
        ($func: expr) => {
            executor::block_on(async { $func.await.unwrap() });
        };
    }

    // #####################
    // adapter test
    // #####################

    #[test]
    fn test_adapter_insert() {
        adapter_insert_test(MemoryAdapter::new());
        adapter_insert_test(RocksAdapter::new("rocksdb/test_adapter_insert".to_string()).unwrap())
    }

    #[test]
    fn test_adapter_batch_modify() {
        adapter_batch_modify_test(MemoryAdapter::new());
        adapter_batch_modify_test(
            RocksAdapter::new("rocksdb/test_adapter_batch_modify".to_string()).unwrap(),
        )
    }

    #[test]
    fn test_adapter_remove() {
        adapter_remove_test(MemoryAdapter::new());
        adapter_remove_test(RocksAdapter::new("rocksdb/test_adapter_remove".to_string()).unwrap())
    }

    // #####################
    // storage test
    // #####################

    #[test]
    fn test_storage_epoch_insert() {
        let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

        let epoch_id = 100;
        let mut epoch = mock_epoch(epoch_id, Hash::digest(get_random_bytes(10)));
        let epoch_hash = Hash::digest(exec!(epoch.header.encode()));

        exec!(storage.insert_epoch(epoch));

        let epoch = exec!(storage.get_latest_epoch());
        assert_eq!(epoch_id, epoch.header.epoch_id);

        let epoch = exec!(storage.get_epoch_by_epoch_id(epoch_id));
        assert_eq!(epoch_id, epoch.header.epoch_id);

        let epoch = exec!(storage.get_epoch_by_hash(epoch_hash));
        assert_eq!(epoch_id, epoch.header.epoch_id);
    }

    #[test]
    fn test_storage_receipts_insert() {
        let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

        let mut receipts = Vec::new();
        let mut hashes = Vec::new();

        for _ in 0..10 {
            let tx_hash = Hash::digest(get_random_bytes(10));
            hashes.push(tx_hash.clone());
            let receipt = mock_receipt(tx_hash.clone());
            receipts.push(receipt);
        }

        exec!(storage.insert_receipts(receipts.clone()));
        let receipts_2 = exec!(storage.get_receipts(hashes));

        for i in 0..10 {
            assert_eq!(
                receipts.get(i).unwrap().tx_hash,
                receipts_2.get(i).unwrap().tx_hash
            );
        }
    }

    #[test]
    fn test_storage_transactions_insert() {
        let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

        let mut transactions = Vec::new();
        let mut hashes = Vec::new();

        for _ in 0..10 {
            let tx_hash = Hash::digest(get_random_bytes(10));
            hashes.push(tx_hash.clone());
            let transaction = mock_signed_tx(tx_hash.clone());
            transactions.push(transaction);
        }

        exec!(storage.insert_transactions(transactions.clone()));
        let transactions_2 = exec!(storage.get_transactions(hashes));

        for i in 0..10 {
            assert_eq!(
                transactions.get(i).unwrap().tx_hash,
                transactions_2.get(i).unwrap().tx_hash
            );
        }
    }

    #[test]
    fn test_storage_latest_proof_insert() {
        let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

        let epoch_hash = Hash::digest(get_random_bytes(10));
        let proof = mock_proof(epoch_hash);

        exec!(storage.update_latest_proof(proof.clone()));
        let proof_2 = exec!(storage.get_latest_proof());

        assert_eq!(proof.epoch_hash, proof_2.epoch_hash);
    }

    // #####################
    // utils
    // #####################

    fn mock_signed_tx(tx_hash: Hash) -> SignedTransaction {
        let nonce = Hash::digest(Bytes::from("XXXX"));
        let fee = Fee {
            asset_id: nonce.clone(),
            cycle:    10,
        };
        let addr_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";

        let action = TransactionAction::Transfer {
            receiver: AccountAddress::from_hex(addr_str).unwrap(),
            asset_id: nonce.clone(),
            amount:   FromPrimitive::from_i32(10).unwrap(),
        };
        let raw = RawTransaction {
            chain_id: nonce.clone(),
            nonce,
            timeout: 10,
            fee,
            action,
        };

        SignedTransaction {
            raw,
            tx_hash,
            pubkey: Default::default(),
            signature: Default::default(),
        }
    }

    fn mock_receipt(tx_hash: Hash) -> Receipt {
        let nonce = Hash::digest(Bytes::from("XXXX"));
        let cycles_used = Fee {
            asset_id: nonce.clone(),
            cycle:    10,
        };
        let addr_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let result = ReceiptResult::Transfer {
            receiver:      AccountAddress::from_hex(addr_str).unwrap(),
            before_amount: FromPrimitive::from_i32(10).unwrap(),
            after_amount:  FromPrimitive::from_i32(20).unwrap(),
        };

        Receipt {
            state_root: nonce.clone(),
            epoch_id: 10,
            tx_hash,
            cycles_used,
            result,
        }
    }

    fn mock_epoch(epoch_id: u64, epoch_hash: Hash) -> Epoch {
        let nonce = Hash::digest(Bytes::from("XXXX"));
        let addr_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let header = EpochHeader {
            chain_id: nonce.clone(),
            epoch_id,
            pre_hash: nonce.clone(),
            timestamp: 1000,
            logs_bloom: Default::default(),
            order_root: nonce.clone(),
            confirm_root: Vec::new(),
            state_root: nonce.clone(),
            receipt_root: Vec::new(),
            cycles_used: 100,
            proposer: AccountAddress::from_hex(addr_str).unwrap(),
            proof: mock_proof(epoch_hash),
            validator_version: 1,
            validators: Vec::new(),
        };

        Epoch {
            header,
            ordered_tx_hashes: Vec::new(),
        }
    }

    fn mock_proof(epoch_hash: Hash) -> Proof {
        Proof {
            epoch_id: 0,
            round: 0,
            epoch_hash,
            signature: Default::default(),
        }
    }

    fn get_random_bytes(len: usize) -> Bytes {
        let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
        Bytes::from(vec)
    }

    fn adapter_insert_test(db: impl StorageAdapter) {
        let tx_hash = Hash::digest(get_random_bytes(10));
        let stx = mock_signed_tx(tx_hash.clone());

        exec!(db.insert::<TransactionSchema>(tx_hash.clone(), stx.clone()));
        let stx = exec!(db.get::<TransactionSchema>(tx_hash.clone())).unwrap();

        assert_eq!(tx_hash, stx.tx_hash);
    }

    fn adapter_batch_modify_test(db: impl StorageAdapter) {
        let mut stxs = Vec::new();
        let mut hashes = Vec::new();
        let mut inserts = Vec::new();

        for _ in 0..10 {
            let tx_hash = Hash::digest(get_random_bytes(10));
            hashes.push(tx_hash.clone());
            let stx = mock_signed_tx(tx_hash.clone());
            stxs.push(stx.clone());
            inserts.push(StorageBatchModify::Insert::<TransactionSchema>(stx));
        }

        exec!(db.batch_modify::<TransactionSchema>(hashes.clone(), inserts));
        let opt_stxs = exec!(db.get_batch::<TransactionSchema>(hashes));

        for i in 0..10 {
            assert_eq!(
                stxs.get(i).unwrap().tx_hash,
                opt_stxs.get(i).unwrap().as_ref().unwrap().tx_hash
            );
        }
    }

    fn adapter_remove_test(db: impl StorageAdapter) {
        let tx_hash = Hash::digest(get_random_bytes(10));
        let is_exist = exec!(db.contains::<TransactionSchema>(tx_hash.clone()));
        assert!(!is_exist);

        let stx = &mock_signed_tx(tx_hash.clone());
        exec!(db.insert::<TransactionSchema>(tx_hash.clone(), stx.clone()));
        let is_exist = exec!(db.contains::<TransactionSchema>(tx_hash.clone()));
        assert!(is_exist);

        exec!(db.remove::<TransactionSchema>(tx_hash.clone()));
        let is_exist = exec!(db.contains::<TransactionSchema>(tx_hash.clone()));
        assert!(!is_exist);
    }
}
