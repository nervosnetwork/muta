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
        let mempool =
            Arc::new(new_mempool($pool_size, TIMEOUT_GAP, CYCLE_LIMIT, MAX_TX_SIZE).await);
        let txs = mock_txs($valid, $invalid, TIMEOUT);
        for _ in 0..$repeat {
            concurrent_insert(txs.clone(), Arc::clone(&mempool)).await;
        }
        assert_eq!(mempool.get_tx_cache().len().await, $output);
    };
}

#[test]
fn test_dup_order_hashes() {
    let hashes = vec![
        Hash::digest(Bytes::from("test1")),
        Hash::digest(Bytes::from("test2")),
        Hash::digest(Bytes::from("test3")),
        Hash::digest(Bytes::from("test4")),
        Hash::digest(Bytes::from("test2")),
    ];
    assert_eq!(check_dup_order_hashes(&hashes).is_err(), true);

    let hashes = vec![
        Hash::digest(Bytes::from("test1")),
        Hash::digest(Bytes::from("test2")),
        Hash::digest(Bytes::from("test3")),
        Hash::digest(Bytes::from("test4")),
    ];
    assert_eq!(check_dup_order_hashes(&hashes).is_err(), false);
}

#[tokio::test]
async fn test_insert() {
    // 1. insertion under pool size.
    insert!(normal(100, 100, 100));

    // 3. invalid insertion
    insert!(invalid(80, 10, 80));
}

macro_rules! package {
    (normal($tx_num_limit: expr, $insert: expr, $expect_order: expr, $expect_propose: expr)) => {
        package!(inner(
            $tx_num_limit,
            TIMEOUT_GAP,
            TIMEOUT,
            $insert,
            $expect_order,
            $expect_propose
        ));
    };
    (timeout($timeout_gap: expr, $timeout: expr, $insert: expr, $expect: expr)) => {
        package!(inner($insert, $timeout_gap, $timeout, $insert, $expect, 0));
    };
    (inner($tx_num_limit: expr, $timeout_gap: expr, $timeout: expr, $insert: expr, $expect_order: expr, $expect_propose: expr)) => {
        let mempool =
            &Arc::new(new_mempool($insert * 10, $timeout_gap, CYCLE_LIMIT, MAX_TX_SIZE).await);
        let txs = mock_txs($insert, 0, $timeout);
        concurrent_insert(txs.clone(), Arc::clone(mempool)).await;
        let mixed_tx_hashes = exec_package(Arc::clone(mempool), CYCLE_LIMIT, $tx_num_limit).await;
        assert_eq!(mixed_tx_hashes.order_tx_hashes.len(), $expect_order);
        assert_eq!(mixed_tx_hashes.propose_tx_hashes.len(), $expect_propose);
    };
}

#[tokio::test]
async fn test_package() {
    // 1. pool_size <= tx_num_limit
    package!(normal(100, 50, 50, 0));
    package!(normal(100, 100, 100, 0));

    // 2. tx_num_limit < pool_size <= 2 * tx_num_limit
    package!(normal(100, 101, 100, 1));
    package!(normal(100, 200, 100, 100));

    // 3. 2 * tx_num_limit < pool_size
    package!(normal(100, 201, 100, 100));

    // 4. current_height >= tx.timeout
    package!(timeout(50, CURRENT_HEIGHT, 10, 0));
    package!(timeout(50, CURRENT_HEIGHT - 10, 10, 0));

    // 5. current_height + timeout_gap < tx.timeout
    package!(timeout(50, CURRENT_HEIGHT + 51, 10, 0));
    package!(timeout(50, CURRENT_HEIGHT + 60, 10, 0));

    // 6. tx.timeout - timeout_gap =< current_height < tx.timeout
    package!(timeout(50, CURRENT_HEIGHT + 50, 10, 10));
    package!(timeout(50, CURRENT_HEIGHT + 1, 10, 10));
}

#[tokio::test]
async fn test_package_order_consistent_with_insert_order() {
    let mempool = &Arc::new(default_mempool().await);

    let txs = default_mock_txs(100);
    for tx in txs.iter() {
        exec_insert(tx.clone(), Arc::clone(mempool)).await;
    }
    let mixed_tx_hashes = exec_package(Arc::clone(mempool), CYCLE_LIMIT, TX_NUM_LIMIT).await;
    assert!(check_order_consistant(&mixed_tx_hashes, &txs));

    // flush partial txs and test order consistency
    let (remove_txs, reserve_txs) = txs.split_at(50);
    let remove_hashes: Vec<Hash> = remove_txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    exec_flush(remove_hashes, Arc::clone(mempool)).await;
    let mixed_tx_hashes = exec_package(Arc::clone(mempool), CYCLE_LIMIT, TX_NUM_LIMIT).await;
    assert!(check_order_consistant(&mixed_tx_hashes, reserve_txs));
}

#[tokio::test]
async fn test_flush() {
    let mempool = Arc::new(default_mempool().await);

    // insert txs
    let txs = default_mock_txs(555);
    concurrent_insert(txs.clone(), Arc::clone(&mempool)).await;
    assert_eq!(mempool.get_tx_cache().len().await, 555);

    let callback_cache = mempool.get_callback_cache();
    for tx in txs.iter() {
        callback_cache.insert(tx.tx_hash.clone(), tx.clone()).await;
    }
    assert_eq!(callback_cache.len().await, 555);

    // flush exist txs
    let (remove_txs, _) = txs.split_at(123);
    let remove_hashes: Vec<Hash> = remove_txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    exec_flush(remove_hashes, Arc::clone(&mempool)).await;
    assert_eq!(mempool.get_tx_cache().len().await, 432);
    assert_eq!(mempool.get_tx_cache().queue_len(), 432);
    exec_package(Arc::clone(&mempool), CYCLE_LIMIT, TX_NUM_LIMIT).await;
    assert_eq!(mempool.get_tx_cache().queue_len(), 432);
    assert_eq!(callback_cache.len().await, 0);

    // flush absent txs
    let txs = default_mock_txs(222);
    let remove_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    exec_flush(remove_hashes, Arc::clone(&mempool)).await;
    assert_eq!(mempool.get_tx_cache().len().await, 432);
    assert_eq!(mempool.get_tx_cache().queue_len(), 432);
}

macro_rules! ensure_order_txs {
    ($in_pool: expr, $out_pool: expr) => {
        let mempool = &Arc::new(default_mempool().await);

        let txs = &default_mock_txs($in_pool + $out_pool);
        let (in_pool_txs, out_pool_txs) = txs.split_at($in_pool);
        concurrent_insert(in_pool_txs.to_vec(), Arc::clone(mempool)).await;
        concurrent_broadcast(out_pool_txs.to_vec(), Arc::clone(mempool)).await;

        let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        exec_ensure_order_txs(tx_hashes.clone(), Arc::clone(mempool)).await;

        assert_eq!(mempool.get_callback_cache().len().await, $out_pool);

        let fetch_txs = exec_get_full_txs(tx_hashes, Arc::clone(mempool)).await;
        assert_eq!(fetch_txs.len(), txs.len());
    };
}

#[tokio::test]
async fn test_ensure_order_txs() {
    // all txs are in pool
    ensure_order_txs!(100, 0);
    // 50 txs are not in pool
    ensure_order_txs!(50, 50);
    // all txs are not in pool
    ensure_order_txs!(0, 100);
}

#[tokio::test]
async fn test_sync_propose_txs() {
    let mempool = &Arc::new(default_mempool().await);

    let txs = &default_mock_txs(50);
    let (exist_txs, need_sync_txs) = txs.split_at(20);
    concurrent_insert(exist_txs.to_vec(), Arc::clone(mempool)).await;
    concurrent_broadcast(need_sync_txs.to_vec(), Arc::clone(mempool)).await;

    let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    exec_sync_propose_txs(tx_hashes, Arc::clone(mempool)).await;

    assert_eq!(mempool.get_tx_cache().len().await, 50);
}

#[rustfmt::skip]
/// Bench in Intel(R) Core(TM) i7-4770HQ CPU @ 2.20GHz (8 x 2200):
/// test tests::mempool::bench_check_sig             ... bench:   2,881,140 ns/iter (+/- 907,215)
/// test tests::mempool::bench_check_sig_serial_1    ... bench:      94,666 ns/iter (+/- 11,070)
/// test tests::mempool::bench_check_sig_serial_10   ... bench:     966,800 ns/iter (+/- 97,227)
/// test tests::mempool::bench_check_sig_serial_100  ... bench:  10,098,216 ns/iter (+/- 1,289,584)
/// test tests::mempool::bench_check_sig_serial_1000 ... bench: 100,396,727 ns/iter (+/- 10,665,143)
/// test tests::mempool::bench_flush                 ... bench:   3,504,193 ns/iter (+/- 1,096,699)
/// test tests::mempool::bench_get_10000_full_txs    ... bench:  14,997,762 ns/iter (+/- 2,697,725)
/// test tests::mempool::bench_get_20000_full_txs    ... bench:  31,858,720 ns/iter (+/- 3,822,648)
/// test tests::mempool::bench_get_40000_full_txs    ... bench:  65,027,639 ns/iter (+/- 3,926,768)
/// test tests::mempool::bench_get_80000_full_txs    ... bench: 131,066,149 ns/iter (+/- 11,457,417)
/// test tests::mempool::bench_insert                ... bench:   9,320,879 ns/iter (+/- 710,246)
/// test tests::mempool::bench_insert_serial_1       ... bench:       4,588 ns/iter (+/- 349)
/// test tests::mempool::bench_insert_serial_10      ... bench:      44,027 ns/iter (+/- 4,168)
/// test tests::mempool::bench_insert_serial_100     ... bench:     432,974 ns/iter (+/- 43,058)
/// test tests::mempool::bench_insert_serial_1000    ... bench:   4,449,648 ns/iter (+/- 560,818)
/// test tests::mempool::bench_mock_txs              ... bench:   5,890,752 ns/iter (+/- 583,029)
/// test tests::mempool::bench_package               ... bench:   3,684,431 ns/iter (+/- 278,575)
/// test tx_cache::tests::bench_flush                ... bench:   3,034,868 ns/iter (+/- 371,514)
/// test tx_cache::tests::bench_flush_insert         ... bench:   2,954,223 ns/iter (+/- 389,002)
/// test tx_cache::tests::bench_gen_txs              ... bench:   2,479,226 ns/iter (+/- 399,728)
/// test tx_cache::tests::bench_insert               ... bench:   2,742,422 ns/iter (+/- 641,587)
/// test tx_cache::tests::bench_package              ... bench:      70,563 ns/iter (+/- 16,723)
/// test tx_cache::tests::bench_package_insert       ... bench:   2,654,196 ns/iter (+/- 285,460)

#[bench]
fn bench_insert(b: &mut Bencher) {
    let mut runtime = tokio::runtime::Runtime::new().unwrap();
    let mempool = &Arc::new(default_mempool_sync());

    b.iter(|| {
        let txs = default_mock_txs(100);
        runtime.block_on(concurrent_insert(txs, Arc::clone(mempool)));
    });
}

#[bench]
fn bench_insert_serial_1(b: &mut Bencher) {
    let mempool = &Arc::new(default_mempool_sync());
    let txs = default_mock_txs(1);

    b.iter(move || {
        futures::executor::block_on(async {
            for tx in txs.clone().into_iter() {
                let _ = mempool.insert(Context::new(), tx).await;
            }
        });
    })
}

#[bench]
fn bench_insert_serial_10(b: &mut Bencher) {
    let mempool = &Arc::new(default_mempool_sync());
    let txs = default_mock_txs(10);

    b.iter(move || {
        futures::executor::block_on(async {
            for tx in txs.clone().into_iter() {
                let _ = mempool.insert(Context::new(), tx).await;
            }
        });
    })
}

#[bench]
fn bench_insert_serial_100(b: &mut Bencher) {
    let mempool = &Arc::new(default_mempool_sync());
    let txs = default_mock_txs(100);

    b.iter(move || {
        futures::executor::block_on(async {
            for tx in txs.clone().into_iter() {
                let _ = mempool.insert(Context::new(), tx).await;
            }
        });
    })
}

#[bench]
fn bench_insert_serial_1000(b: &mut Bencher) {
    let mempool = &Arc::new(default_mempool_sync());
    let txs = default_mock_txs(1000);

    b.iter(move || {
        futures::executor::block_on(async {
            for tx in txs.clone().into_iter() {
                let _ = mempool.insert(Context::new(), tx).await;
            }
        });
    })
}

#[bench]
fn bench_package(b: &mut Bencher) {
    let mut runtime = tokio::runtime::Runtime::new().unwrap();

    let mempool = Arc::new(default_mempool_sync());
    let txs = default_mock_txs(50_000);
    runtime.block_on(concurrent_insert(txs, Arc::clone(&mempool)));
    b.iter(|| {
        runtime.block_on(exec_package(
            Arc::clone(&mempool),
            CYCLE_LIMIT,
            TX_NUM_LIMIT,
        ));
    });
}

#[bench]
fn bench_get_10000_full_txs(b: &mut Bencher) {
    let mut runtime = tokio::runtime::Runtime::new().unwrap();

    let mempool = Arc::new(default_mempool_sync());
    let txs = default_mock_txs(10_000);
    let tx_hashes = txs.iter().map(|tx| tx.tx_hash.clone()).collect::<Vec<_>>();
    runtime.block_on(concurrent_insert(txs, Arc::clone(&mempool)));
    b.iter(|| {
        runtime.block_on(exec_get_full_txs(tx_hashes.clone(), Arc::clone(&mempool)));
    });
}

#[bench]
fn bench_get_20000_full_txs(b: &mut Bencher) {
    let mut runtime = tokio::runtime::Runtime::new().unwrap();

    let mempool = Arc::new(default_mempool_sync());
    let txs = default_mock_txs(20_000);
    let tx_hashes = txs.iter().map(|tx| tx.tx_hash.clone()).collect::<Vec<_>>();
    runtime.block_on(concurrent_insert(txs, Arc::clone(&mempool)));
    b.iter(|| {
        runtime.block_on(exec_get_full_txs(tx_hashes.clone(), Arc::clone(&mempool)));
    });
}

#[bench]
fn bench_get_40000_full_txs(b: &mut Bencher) {
    let mut runtime = tokio::runtime::Runtime::new().unwrap();

    let mempool = Arc::new(default_mempool_sync());
    let txs = default_mock_txs(40_000);
    let tx_hashes = txs.iter().map(|tx| tx.tx_hash.clone()).collect::<Vec<_>>();
    runtime.block_on(concurrent_insert(txs, Arc::clone(&mempool)));
    b.iter(|| {
        runtime.block_on(exec_get_full_txs(tx_hashes.clone(), Arc::clone(&mempool)));
    });
}

#[bench]
fn bench_get_80000_full_txs(b: &mut Bencher) {
    let mut runtime = tokio::runtime::Runtime::new().unwrap();

    let mempool = Arc::new(default_mempool_sync());
    let txs = default_mock_txs(80_000);
    let tx_hashes = txs.iter().map(|tx| tx.tx_hash.clone()).collect::<Vec<_>>();
    runtime.block_on(concurrent_insert(txs, Arc::clone(&mempool)));
    b.iter(|| {
        runtime.block_on(exec_get_full_txs(tx_hashes.clone(), Arc::clone(&mempool)));
    });
}

#[bench]
fn bench_flush(b: &mut Bencher) {
    let mut runtime = tokio::runtime::Runtime::new().unwrap();

    let mempool = &Arc::new(default_mempool_sync());
    let txs = &default_mock_txs(100);
    let remove_hashes: &Vec<Hash> = &txs.iter().map(|tx| tx.tx_hash.clone()).collect();
    b.iter(|| {
        runtime.block_on(concurrent_insert(txs.clone(), Arc::clone(mempool)));
        runtime.block_on(exec_flush(remove_hashes.clone(), Arc::clone(mempool)));
        runtime.block_on(exec_package(Arc::clone(mempool), CYCLE_LIMIT, TX_NUM_LIMIT));
    });
}

#[tokio::test]
async fn bench_sign_with_spawn_list() {
    let adapter = Arc::new(HashMemPoolAdapter::new());
    let txs = default_mock_txs(30000);
    let len = txs.len();
    let now = std::time::Instant::now();

    let futs = txs
        .into_iter()
        .map(|tx| {
            let adapter = Arc::clone(&adapter);
            tokio::spawn(async move {
                adapter
                    .check_authorization(Context::new(), Box::new(tx))
                    .await
                    .unwrap();
            })
        })
        .collect::<Vec<_>>();
    futures::future::try_join_all(futs).await.unwrap();

    println!(
        "bench_sign_with_spawn_list size {:?} cost {:?}",
        len,
        now.elapsed()
    );
}

#[tokio::test]
async fn bench_sign() {
    let adapter = HashMemPoolAdapter::new();
    let txs = default_mock_txs(30000)
        .into_iter()
        .map(Box::new)
        .collect::<Vec<_>>();
    let now = std::time::Instant::now();

    for tx in txs.iter() {
        adapter
            .check_authorization(Context::new(), tx.clone())
            .await
            .unwrap();
    }

    println!("bench_sign size {:?} cost {:?}", txs.len(), now.elapsed());
}

#[bench]
fn bench_mock_txs(b: &mut Bencher) {
    b.iter(|| {
        default_mock_txs(100);
    });
}

#[bench]
fn bench_check_sig(b: &mut Bencher) {
    let mut runtime = tokio::runtime::Runtime::new().unwrap();

    let txs = &default_mock_txs(100);

    b.iter(|| {
        runtime.block_on(concurrent_check_sig(txs.clone()));
    });
}

#[bench]
fn bench_check_sig_serial_1(b: &mut Bencher) {
    let txs = default_mock_txs(1);

    b.iter(|| {
        for tx in txs.iter() {
            let _ = check_sig(&tx);
        }
    })
}

#[bench]
fn bench_check_sig_serial_10(b: &mut Bencher) {
    let txs = default_mock_txs(10);

    b.iter(|| {
        for tx in txs.iter() {
            let _ = check_sig(&tx);
        }
    })
}

#[bench]
fn bench_check_sig_serial_100(b: &mut Bencher) {
    let txs = default_mock_txs(100);

    b.iter(|| {
        for tx in txs.iter() {
            let _ = check_sig(&tx);
        }
    })
}

#[bench]
fn bench_check_sig_serial_1000(b: &mut Bencher) {
    let txs = default_mock_txs(1000);

    b.iter(|| {
        for tx in txs.iter() {
            let _ = check_sig(&tx);
        }
    })
}
