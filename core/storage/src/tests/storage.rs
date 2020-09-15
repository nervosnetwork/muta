extern crate test;

use std::sync::Arc;

use test::Bencher;

use protocol::traits::{CommonStorage, Context, Storage};
use protocol::types::Hash;
use tokio::runtime::Runtime;

use crate::adapter::memory::MemoryAdapter;
use crate::tests::{get_random_bytes, mock_block, mock_proof, mock_receipt, mock_signed_tx};
use crate::ImplStorage;
use crate::BATCH_VALUE_DECODE_NUMBER;

#[tokio::test]
async fn test_storage_block_insert() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let height = 100;
    let block = mock_block(height, Hash::digest(get_random_bytes(10)));

    storage.insert_block(Context::new(), block).await.unwrap();

    let block = storage.get_latest_block(Context::new()).await.unwrap();
    assert_eq!(height, block.header.height);

    let block = storage.get_block(Context::new(), height).await.unwrap();
    assert_eq!(Some(height), block.map(|b| b.header.height));
}

#[tokio::test]
async fn test_storage_receipts_insert() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2077;

    let mut receipts = Vec::new();
    let mut hashes = Vec::new();

    for _ in 0..10 {
        let tx_hash = Hash::digest(get_random_bytes(10));
        hashes.push(tx_hash.clone());
        let receipt = mock_receipt(tx_hash.clone());
        receipts.push(receipt);
    }

    storage
        .insert_receipts(Context::new(), height, receipts.clone())
        .await
        .unwrap();
    let receipts_2 = storage
        .get_receipts(Context::new(), height, hashes)
        .await
        .unwrap();

    for i in 0..10 {
        assert_eq!(
            Some(receipts.get(i).unwrap()),
            receipts_2.get(i).unwrap().as_ref()
        );
    }
}

#[tokio::test]
async fn test_storage_receipts_get_batch_decode() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2077;
    let count = BATCH_VALUE_DECODE_NUMBER + 100;

    let mut receipts = Vec::new();
    let mut hashes = Vec::new();

    for _ in 0..count {
        let tx_hash = Hash::digest(get_random_bytes(10));
        hashes.push(tx_hash.clone());
        let receipt = mock_receipt(tx_hash.clone());
        receipts.push(receipt);
    }

    storage
        .insert_receipts(Context::new(), height, receipts.clone())
        .await
        .unwrap();

    let receipts_2 = storage
        .get_receipts(Context::new(), height, hashes)
        .await
        .unwrap();

    for i in 0..count {
        assert_eq!(
            Some(receipts.get(i).unwrap()),
            receipts_2.get(i).unwrap().as_ref()
        );
    }
}

#[tokio::test]
async fn test_storage_transactions_insert() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2020;

    let mut transactions = Vec::new();
    let mut hashes = Vec::new();

    for _ in 0..10 {
        let tx_hash = Hash::digest(get_random_bytes(10));
        hashes.push(tx_hash.clone());
        let transaction = mock_signed_tx(tx_hash.clone());
        transactions.push(transaction);
    }

    storage
        .insert_transactions(Context::new(), height, transactions.clone())
        .await
        .unwrap();
    let transactions_2 = storage
        .get_transactions(Context::new(), height, hashes)
        .await
        .unwrap();

    for i in 0..10 {
        assert_eq!(
            Some(transactions.get(i).unwrap()),
            transactions_2.get(i).unwrap().as_ref()
        );
    }
}

#[tokio::test]
async fn test_storage_transactions_get_batch_decode() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2020;
    let count = BATCH_VALUE_DECODE_NUMBER + 100;

    let mut transactions = Vec::new();
    let mut hashes = Vec::new();

    for _ in 0..count {
        let tx_hash = Hash::digest(get_random_bytes(10));
        hashes.push(tx_hash.clone());
        let transaction = mock_signed_tx(tx_hash.clone());
        transactions.push(transaction);
    }

    storage
        .insert_transactions(Context::new(), height, transactions.clone())
        .await
        .unwrap();
    let transactions_2 = storage
        .get_transactions(Context::new(), height, hashes)
        .await
        .unwrap();

    for i in 0..count {
        assert_eq!(
            Some(transactions.get(i).unwrap()),
            transactions_2.get(i).unwrap().as_ref()
        );
    }
}

#[tokio::test]
async fn test_storage_latest_proof_insert() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let block_hash = Hash::digest(get_random_bytes(10));
    let proof = mock_proof(block_hash);

    storage
        .update_latest_proof(Context::new(), proof.clone())
        .await
        .unwrap();
    let proof_2 = storage.get_latest_proof(Context::new()).await.unwrap();

    assert_eq!(proof.block_hash, proof_2.block_hash);
}

#[rustfmt::skip]
/// Bench in Intel(R) Core(TM) i7-4770HQ CPU @ 2.20GHz (8 x 2200)
/// test tests::storage::bench_insert_10000_receipts ... bench:  33,954,916 ns/iter (+/- 3,818,780)
/// test tests::storage::bench_insert_20000_receipts ... bench:  69,476,334 ns/iter (+/- 25,206,468)
/// test tests::storage::bench_insert_40000_receipts ... bench: 138,903,121 ns/iter (+/- 26,053,433)
/// test tests::storage::bench_insert_80000_receipts ... bench: 289,629,756 ns/iter (+/- 114,583,692)
/// test tests::storage::bench_insert_10000_txs      ... bench:  37,900,652 ns/iter (+/- 19,055,351)
/// test tests::storage::bench_insert_20000_txs      ... bench:  76,499,664 ns/iter (+/- 17,883,127)
/// test tests::storage::bench_insert_40000_txs      ... bench: 148,111,340 ns/iter (+/- 5,637,411)
/// test tests::storage::bench_insert_80000_txs      ... bench: 311,861,163 ns/iter (+/- 16,891,290)

#[bench]
fn bench_insert_10000_receipts(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2045;

    let receipts = (0..10000)
        .map(|_| mock_receipt(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    let mut rt = Runtime::new().unwrap();
    b.iter(|| {
        rt.block_on(storage.insert_receipts(Context::new(), height, receipts.clone())).unwrap()
    })
}

#[bench]
fn bench_insert_20000_receipts(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2045;

    let receipts = (0..20000)
        .map(|_| mock_receipt(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    let mut rt = Runtime::new().unwrap();
    b.iter(move || {
        rt.block_on(storage.insert_receipts(Context::new(), height, receipts.clone()))
            .unwrap()
    })
}

#[bench]
fn bench_insert_40000_receipts(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2077;

    let receipts = (0..40000)
        .map(|_| mock_receipt(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    let mut rt = Runtime::new().unwrap();
    b.iter(move || {
        rt.block_on(storage.insert_receipts(Context::new(), height, receipts.clone()))
            .unwrap()
    })
}

#[bench]
fn bench_insert_80000_receipts(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2077;

    let receipts = (0..80000)
        .map(|_| mock_receipt(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    let mut rt = Runtime::new().unwrap();
    b.iter(move || {
        rt.block_on(storage.insert_receipts(Context::new(), height, receipts.clone()))
            .unwrap()
    })
}
#[bench]
fn bench_insert_10000_txs(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2077;

    let txs = (0..10000)
        .map(|_| mock_signed_tx(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    let mut rt = Runtime::new().unwrap();
    b.iter(move || {
        rt.block_on(storage.insert_transactions(Context::new(), height, txs.clone()))
            .unwrap()
    })
}

#[bench]
fn bench_insert_20000_txs(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2077;

    let txs = (0..20000)
        .map(|_| mock_signed_tx(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    let mut rt = Runtime::new().unwrap();
    b.iter(move || {
        rt.block_on(storage.insert_transactions(Context::new(), height, txs.clone()))
            .unwrap()
    })
}

#[bench]
fn bench_insert_40000_txs(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2077;

    let txs = (0..40000)
        .map(|_| mock_signed_tx(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    let mut rt = Runtime::new().unwrap();
    b.iter(move || {
        rt.block_on(storage.insert_transactions(Context::new(), height, txs.clone()))
            .unwrap()
    })
}

#[bench]
fn bench_insert_80000_txs(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));
    let height = 2077;

    let txs = (0..80000)
        .map(|_| mock_signed_tx(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    let mut rt = Runtime::new().unwrap();
    b.iter(move || {
        rt.block_on(storage.insert_transactions(Context::new(), height, txs.clone()))
            .unwrap()
    })
}
