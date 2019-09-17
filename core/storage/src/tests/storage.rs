use std::sync::Arc;

use protocol::codec::ProtocolCodec;
use protocol::traits::Storage;
use protocol::types::Hash;

use crate::adapter::memory::MemoryAdapter;
use crate::tests::{get_random_bytes, mock_epoch, mock_proof, mock_receipt, mock_signed_tx};
use crate::ImplStorage;

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
