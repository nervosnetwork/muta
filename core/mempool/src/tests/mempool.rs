use std::sync::Arc;

use test::Bencher;

use protocol::types::Hash;

use super::*;

macro_rules! insert {
    (normal($pool_size: expr, $input: expr, $output: expr)) => {
        insert!(inner($pool_size, 1, $input, 0, $output));
    };
    (repeat($repeat: expr, $input: expr, $output: expr)) => {
        insert!(inner($input * 10, $repeat, $input, 0, $output));
    };
    (invalid($valid: expr, $invalid: expr, $output: expr)) => {
        insert!(inner($valid * 10, 1, $valid, $invalid, $output));
    };
    (inner($pool_size: expr, $repeat: expr, $valid: expr, $invalid: expr, $output: expr)) => {
        let mempool = Arc::new(new_mempool($pool_size, TIMEOUT_GAP, CURRENT_EPOCH_ID));
        let txs = mock_txs($valid, $invalid, TIMEOUT);
        for _ in 0..$repeat {
            concurrent_insert(txs.clone(), Arc::clone(&mempool));
        }
        assert_eq!(mempool.get_tx_cache().len(), $output);
    };
}

#[test]
fn test_insert() {
    // 1. insertion under pool size.
    insert!(normal(100, 100, 100));

    // 2. insertion above pool size.
    insert!(normal(100, 101, 100));

    // 3. repeat insertion
    insert!(repeat(5, 200, 200));

    // 4. invalid insertion
    insert!(invalid(80, 10, 80));
}

macro_rules! package {
    (normal($cycle_limit: expr, $insert: expr, $expect_order: expr, $expect_propose: expr)) => {
        package!(inner(
            $cycle_limit,
            CURRENT_EPOCH_ID,
            TIMEOUT_GAP,
            TIMEOUT,
            $insert,
            $expect_order,
            $expect_propose
        ));
    };
    (timeout($current_epoch_id: expr, $timeout_gap: expr, $timeout: expr, $insert: expr, $expect: expr)) => {
        package!(inner(
            $insert,
            $current_epoch_id,
            $timeout_gap,
            $timeout,
            $insert,
            $expect,
            0
        ));
    };
    (inner($cycle_limit: expr, $current_epoch_id: expr, $timeout_gap: expr, $timeout: expr, $insert: expr, $expect_order: expr, $expect_propose: expr)) => {
        let mempool = &Arc::new(new_mempool($insert * 10, $timeout_gap, $current_epoch_id));
        let txs = mock_txs($insert, 0, $timeout);
        concurrent_insert(txs.clone(), Arc::clone(mempool));
        let mixed_tx_hashes = exec_package(Arc::clone(mempool), $cycle_limit);
        assert_eq!(mixed_tx_hashes.order_tx_hashes.len(), $expect_order);
        assert_eq!(mixed_tx_hashes.propose_tx_hashes.len(), $expect_propose);
    };
}

#[test]
fn test_package() {
    // 1. pool_size <= cycle_limit
    package!(normal(100, 50, 50, 0));
    package!(normal(100, 100, 100, 0));

    // 2. cycle_limit < pool_size <= 2 * cycle_limit
    package!(normal(100, 101, 100, 1));
    package!(normal(100, 200, 100, 100));

    // 3. 2 * cycle_limit < pool_size
    package!(normal(100, 201, 100, 100));

    // 4. current_epoch_id >= tx.timeout
    package!(timeout(100, 50, 100, 10, 0));
    package!(timeout(100, 50, 90, 10, 0));

    // 5. current_epoch_id + timeout_gap < tx.timeout
    package!(timeout(100, 50, 151, 10, 0));
    package!(timeout(100, 50, 160, 10, 0));

    // 6. tx.timeout - timeout_gap =< current_epoch_id < tx.timeout
    package!(timeout(100, 50, 150, 10, 10));
    package!(timeout(100, 50, 101, 10, 10));
}

#[test]
fn test_package_order_consistent_with_insert_order() {
    let mempool = &Arc::new(default_mempool());

    let txs = &default_mock_txs(100);
    txs.iter()
        .for_each(|signed_tx| exec_insert(signed_tx, Arc::clone(mempool)));
    let mixed_tx_hashes = exec_package(Arc::clone(mempool), CYCLE_LIMIT);
    assert!(check_order_consistant(&mixed_tx_hashes, txs));

    // flush partial txs and test order consistency
    let (remove_txs, reserve_txs) = txs.split_at(50);
    let remove_hashes: Vec<Hash> = remove_txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    exec_flush(remove_hashes, Arc::clone(mempool));
    let mixed_tx_hashes = exec_package(Arc::clone(mempool), CYCLE_LIMIT);
    assert!(check_order_consistant(&mixed_tx_hashes, reserve_txs));
}

#[test]
fn test_flush() {
    let mempool = Arc::new(default_mempool());

    // insert txs
    let txs = default_mock_txs(555);
    concurrent_insert(txs.clone(), Arc::clone(&mempool));
    assert_eq!(mempool.get_tx_cache().len(), 555);

    let callback_cache = mempool.get_callback_cache();
    txs.iter().for_each(|tx| {
        callback_cache.insert(tx.tx_hash.clone(), tx.clone());
    });
    assert_eq!(callback_cache.len(), 555);

    // flush exist txs
    let (remove_txs, _) = txs.split_at(123);
    let remove_hashes: Vec<Hash> = remove_txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    exec_flush(remove_hashes, Arc::clone(&mempool));
    assert_eq!(mempool.get_tx_cache().len(), 432);
    assert_eq!(mempool.get_tx_cache().queue_len(), 555);
    exec_package(Arc::clone(&mempool), CYCLE_LIMIT);
    assert_eq!(mempool.get_tx_cache().queue_len(), 432);
    assert_eq!(callback_cache.len(), 0);

    // flush absent txs
    let txs = default_mock_txs(222);
    let remove_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    exec_flush(remove_hashes, Arc::clone(&mempool));
    assert_eq!(mempool.get_tx_cache().len(), 432);
    assert_eq!(mempool.get_tx_cache().queue_len(), 432);
}

macro_rules! ensure_order_txs {
    ($in_pool: expr, $out_pool: expr) => {
        let mempool = &Arc::new(default_mempool());

        let txs = &default_mock_txs($in_pool + $out_pool);
        let (in_pool_txs, out_pool_txs) = txs.split_at($in_pool);
        concurrent_insert(in_pool_txs.to_vec(), Arc::clone(mempool));
        concurrent_broadcast(out_pool_txs.to_vec(), Arc::clone(mempool));

        let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        exec_ensure_order_txs(tx_hashes.clone(), Arc::clone(mempool));

        assert_eq!(mempool.get_callback_cache().len(), $out_pool);

        let fetch_txs = exec_get_full_txs(tx_hashes, Arc::clone(mempool));
        assert_eq!(fetch_txs.len(), txs.len());
    };
}

#[test]
fn test_ensure_order_txs() {
    // all txs are in pool
    ensure_order_txs!(100, 0);
    // 50 txs are not in pool
    ensure_order_txs!(50, 50);
    // all txs are not in pool
    ensure_order_txs!(0, 100);
}

#[test]
fn test_sync_propose_txs() {
    let mempool = &Arc::new(default_mempool());

    let txs = &default_mock_txs(50);
    let (exist_txs, need_sync_txs) = txs.split_at(20);
    concurrent_insert(exist_txs.to_vec(), Arc::clone(mempool));
    concurrent_broadcast(need_sync_txs.to_vec(), Arc::clone(mempool));

    let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    exec_sync_propose_txs(tx_hashes.clone(), Arc::clone(mempool));

    assert_eq!(mempool.get_tx_cache().len(), 50);
}

#[bench]
fn bench_insert(b: &mut Bencher) {
    let mempool = &Arc::new(default_mempool());

    b.iter(|| {
        let txs = default_mock_txs(100);
        concurrent_insert(txs, Arc::clone(mempool));
    });
}

#[bench]
fn bench_package(b: &mut Bencher) {
    let mempool = Arc::new(default_mempool());
    let txs = default_mock_txs(50_000);
    concurrent_insert(txs.clone(), Arc::clone(&mempool));
    b.iter(|| {
        exec_package(Arc::clone(&mempool), CYCLE_LIMIT);
    });
}

#[bench]
fn bench_flush(b: &mut Bencher) {
    let mempool = &Arc::new(default_mempool());
    let txs = &default_mock_txs(100);
    let remove_hashes: &Vec<Hash> = &txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    b.iter(|| {
        concurrent_insert(txs.clone(), Arc::clone(mempool));
        exec_flush(remove_hashes.clone(), Arc::clone(mempool));
        exec_package(Arc::clone(mempool), CYCLE_LIMIT);
    });
}

#[bench]
fn bench_mock_txs(b: &mut Bencher) {
    b.iter(|| {
        default_mock_txs(100);
    });
}

#[bench]
fn bench_check_sig(b: &mut Bencher) {
    let txs = &default_mock_txs(100);

    b.iter(|| {
        concurrent_check_sig(txs.clone());
    });
}
