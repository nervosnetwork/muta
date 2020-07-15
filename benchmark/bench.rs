use asset::types::TransferPayload;

use super::*;

#[rustfmt::skip]
/// Bench in Intel(R) Core(TM) i7-4770HQ CPU @ 2.20GHz (8 x 2200)
/// 100 txs bench_execute ... bench:  11,299,912 ns/iter (+/- 3,402,276)
/// 1000 txs bench::bench_execute ... bench: 101,187,934 ns/iter (+/- 26,000,469)
#[bench]
fn bench_execute(b: &mut Bencher) {
    let payload = TransferPayload {
        asset_id: NATIVE_ASSET_ID.clone(),
        to:       FEE_INLET_ACCOUNT.clone(),
        value:    1u64,
    };

    let req = TransactionRequest {
        service_name: "asset".to_string(),
        method:       "transfer".to_string(),
        payload:      serde_json::to_string(&payload).unwrap(),
    };

    benchmark!(req, 1_000, b);
}
