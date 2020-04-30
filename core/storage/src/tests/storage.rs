use std::fs;
use std::sync::Arc;
use std::time::SystemTime;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::Storage;
use protocol::types::Hash;

use crate::adapter::memory::MemoryAdapter;
use crate::adapter::rocks::{Config, RocksAdapter};
use crate::tests::{get_random_bytes, mock_block, mock_proof, mock_receipt, mock_signed_tx};
use crate::ImplStorage;

#[test]
fn test_storage_block_insert() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let height = 100;
    let block = mock_block(height, Hash::digest(get_random_bytes(10)));
    let block_hash = Hash::digest(block.encode_fixed().unwrap());

    exec!(storage.insert_block(block));

    let block = exec!(storage.get_latest_block());
    assert_eq!(height, block.header.height);

    let block = exec!(storage.get_block_by_height(height));
    assert_eq!(height, block.header.height);

    let block = exec!(storage.get_block_by_hash(block_hash));
    assert_eq!(height, block.header.height);
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

    let block_hash = Hash::digest(get_random_bytes(10));
    let proof = mock_proof(block_hash);

    exec!(storage.update_latest_proof(proof.clone()));
    let proof_2 = exec!(storage.get_latest_proof());

    assert_eq!(proof.block_hash, proof_2.block_hash);
}

#[test]
fn test_storage_wal_insert() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let info = get_random_bytes(64);
    exec!(storage.update_overlord_wal(info.clone()));
    let info_2 = exec!(storage.load_overlord_wal());
    assert_eq!(info, info_2);
}

#[test]
#[ignore]
fn test_storage_stat() {
    fs::remove_dir_all("rocksdb/test_adapter_stat").unwrap();
    let adapter = Arc::new(
        RocksAdapter::new("rocksdb/test_adapter_stat".to_string(), Config::suggest()).unwrap(),
    );
    let storage = Arc::new(ImplStorage::new(Arc::clone(&adapter)));

    let loop_num = 10;
    let size = 1_000_000;
    let rand_size = 500; // 500 * 10 = 5000

    let mut head_5000_hashes = Vec::new();
    let mut tail_5000_hashes = Vec::new();
    let mut rand_5000_hashes = Vec::new();

    for i in 0..loop_num {
        let mut transactions = Vec::new();
        let mut hashes = Vec::new();

        for _ in 0..size {
            let tx_hash = Hash::digest(get_random_bytes(10));
            hashes.push(tx_hash.clone());
            let transaction = mock_signed_tx(tx_hash.clone());
            transactions.push(transaction);
        }
        if i == 0 {
            head_5000_hashes = hashes[0..5000].to_vec();
        }
        if i == loop_num - 1 {
            tail_5000_hashes = hashes[size - 5000..size].to_vec();
        }
        rand_5000_hashes.extend_from_slice(&hashes[0..rand_size]);

        let now = SystemTime::now();
        exec!(storage.insert_transactions(transactions.clone()));
        println!(
            "insert {:?} tx spent {:?}ms",
            size,
            now.elapsed().unwrap().as_millis()
        );
    }

    let now_head = SystemTime::now();
    exec!(storage.get_transactions(head_5000_hashes.to_vec()));
    println!(
        "get head 5000 tx spent {:?}ms",
        now_head.elapsed().unwrap().as_millis()
    );

    let now_tail = SystemTime::now();
    exec!(storage.get_transactions(tail_5000_hashes.to_vec()));
    println!(
        "get tail 5000 tx spent {:?}ms",
        now_tail.elapsed().unwrap().as_millis()
    );

    let now_rand = SystemTime::now();
    exec!(storage.get_transactions(rand_5000_hashes.to_vec()));
    println!(
        "get rand 5000 tx spent {:?}ms",
        now_rand.elapsed().unwrap().as_millis()
    );
}
