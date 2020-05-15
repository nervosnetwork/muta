extern crate test;

use std::sync::Arc;

use test::Bencher;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{Context, Storage};
use protocol::types::Hash;

use crate::adapter::memory::MemoryAdapter;
use crate::tests::{get_random_bytes, mock_block, mock_proof, mock_receipt, mock_signed_tx};
use crate::ImplStorage;

#[test]
fn test_storage_block_insert() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let height = 100;
    let block = mock_block(height, Hash::digest(get_random_bytes(10)));
    let block_hash = Hash::digest(block.header.encode_fixed().unwrap());

    exec!(storage.insert_block(Context::new(), block));

    let block = exec!(storage.get_latest_block(Context::new()));
    assert_eq!(height, block.header.height);

    let block = exec!(storage.get_block_by_height(Context::new(), height));
    assert_eq!(height, block.header.height);

    let block = exec!(storage.get_block_by_hash(Context::new(), block_hash));
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

    exec!(storage.insert_receipts(Context::new(), receipts.clone()));
    let receipts_2 = exec!(storage.get_receipts(Context::new(), hashes));

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

    exec!(storage.insert_transactions(Context::new(), transactions.clone()));
    let transactions_2 = exec!(storage.get_transactions(Context::new(), hashes));

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

    exec!(storage.update_latest_proof(Context::new(), proof.clone()));
    let proof_2 = exec!(storage.get_latest_proof(Context::new(),));

    assert_eq!(proof.block_hash, proof_2.block_hash);
}

#[test]
fn test_storage_wal_insert() {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let info = get_random_bytes(64);
    exec!(storage.update_overlord_wal(Context::new(), info.clone()));
    let info_2 = exec!(storage.load_overlord_wal(Context::new(),));
    assert_eq!(info, info_2);
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

    let receipts = (0..10000)
        .map(|_| mock_receipt(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    b.iter(move || {
        exec!(storage.insert_receipts(Context::new(), receipts.clone()));
    })
}

#[bench]
fn bench_insert_20000_receipts(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let receipts = (0..20000)
        .map(|_| mock_receipt(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    b.iter(move || {
        exec!(storage.insert_receipts(Context::new(), receipts.clone()));
    })
}

#[bench]
fn bench_insert_40000_receipts(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let receipts = (0..40000)
        .map(|_| mock_receipt(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    b.iter(move || {
        exec!(storage.insert_receipts(Context::new(), receipts.clone()));
    })
}

#[bench]
fn bench_insert_80000_receipts(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let receipts = (0..80000)
        .map(|_| mock_receipt(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    b.iter(move || {
        exec!(storage.insert_receipts(Context::new(), receipts.clone()));
    })
}
#[bench]
fn bench_insert_10000_txs(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let txs = (0..10000)
        .map(|_| mock_signed_tx(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    b.iter(move || {
        exec!(storage.insert_transactions(Context::new(), txs.clone()));
    })
}

#[bench]
fn bench_insert_20000_txs(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let txs = (0..20000)
        .map(|_| mock_signed_tx(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    b.iter(move || {
        exec!(storage.insert_transactions(Context::new(), txs.clone()));
    })
}

#[bench]
fn bench_insert_40000_txs(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let txs = (0..40000)
        .map(|_| mock_signed_tx(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    b.iter(move || {
        exec!(storage.insert_transactions(Context::new(), txs.clone()));
    })
}

#[bench]
fn bench_insert_80000_txs(b: &mut Bencher) {
    let storage = ImplStorage::new(Arc::new(MemoryAdapter::new()));

    let txs = (0..80000)
        .map(|_| mock_signed_tx(Hash::digest(get_random_bytes(10))))
        .collect::<Vec<_>>();

    b.iter(move || {
        exec!(storage.insert_transactions(Context::new(), txs.clone()));
    })
}
