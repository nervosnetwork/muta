#![allow(clippy::needles_collect)]

use asset::types::TransferPayload;

use super::*;

#[rustfmt::skip]
/// Bench in Intel(R) Core(TM) i7-4770HQ CPU @ 2.20GHz (8 x 2200)
/// 100 txs bench_execute ... bench:  11,299,912 ns/iter (+/- 3,402,276)
/// 1000 txs bench::bench_execute ... bench: 101,187,934 ns/iter (+/- 26,000,469)
#[bench]
fn bench_execute(b: &mut Bencher) {
    let mut bench_adapter = BenchmarkAdapter::new();

    let payload = TransferPayload {
        asset_id: NATIVE_ASSET_ID.clone(),
        to:       FEE_INLET_ACCOUNT.clone(),
        value:    1u64,
    };

    let req = (0..1000).map(|_| TransactionRequest {
        service_name: "asset".to_string(),
        method:       "transfer".to_string(),
        payload:      serde_json::to_string(&payload).unwrap(),
    }).collect::<Vec<_>>();

    perf_exec!(bench_adapter, req, b);
}

#[rustfmt::skip]
/// 10 assets bench::perf_execute  ... bench: 109,202,563 ns/iter (+/- 6,378,009)
/// 100 assets bench::perf_execute  ... bench: 108,859,512 ns/iter (+/- 2,977,622)
/// 1000 assets bench::bench_execute ... bench: 108,037,404 ns/iter (+/- 4,539,634)
/// 10000 assets test bench::perf_execute  ... bench: 100,244,123 ns/iter (+/- 18,935,087)
#[bench]
fn bench_execute_with_assets(b: &mut Bencher) {
    let mut bench_adapter = BenchmarkAdapter::new();
    create_assets(&mut bench_adapter, 10000);

    let payload = TransferPayload {
        asset_id: NATIVE_ASSET_ID.clone(),
        to:       FEE_INLET_ACCOUNT.clone(),
        value:    1u64,
    };

    let req = (0..1000).map(|_| TransactionRequest {
        service_name: "asset".to_string(),
        method:       "transfer".to_string(),
        payload:      serde_json::to_string(&payload).unwrap(),
    }).collect::<Vec<_>>();

    perf_exec!(bench_adapter, req, b);
}

fn create_assets(bench_adapter: &mut BenchmarkAdapter, num: u64) {
    let create_assets = (0..num)
        .map(|n| {
            let payload = asset::types::CreateAssetPayload {
                name:   "muta_".to_string() + n.to_string().as_str(),
                symbol: "muta_".to_string() + n.to_string().as_str(),
                supply: 100_000,
            };

            TransactionRequest {
                service_name: "asset".to_string(),
                method:       "create_asset".to_string(),
                payload:      serde_json::to_string(&payload).unwrap(),
            }
        })
        .collect::<Vec<_>>();

    exec!(bench_adapter, create_assets);
}
