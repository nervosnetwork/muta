use futures::future::Future;
use muta::{proto::blockchain, service::PoolService};
use std::{thread, time};

mod pool;

fn main() {
    let mut count = 1;
    let sleep_time = time::Duration::from_secs(3);
    let dummy_pool = pool::DummyPool {};

    loop {
        let mut utx = blockchain::UnverifiedTransaction::default();
        let mut tx = blockchain::Transaction::default();
        tx.nonce = format!("{}", count);
        tx.quota = 111;
        tx.valid_until_block = 222;
        tx.data = format!("data {}", count).as_bytes().to_vec();
        utx.transaction = Some(tx);

        let resp = dummy_pool.add_unverified_transaction(Default::default(), utx);
        println!("{:?}", resp.wait());

        count += 1;

        thread::sleep(sleep_time);
    }
}
