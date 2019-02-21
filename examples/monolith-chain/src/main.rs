use futures::future::Future;
use std::{thread, time};
use muta::{proto::blockchain, service::PoolService};

mod pool;

fn main() {
    let mut count = 1;
    let sleep_time = time::Duration::from_secs(3);
    let dummy_pool = pool::DummyPool {};

    loop {
        let mut utx = blockchain::UnverifiedTransaction::new();
        let mut tx = blockchain::Transaction::new();
        tx.set_nonce(format!("{}", count));
        tx.set_quota(111);
        tx.set_valid_until_block(222);
        tx.set_data(format!("data {}", count).as_bytes().to_vec());
        utx.set_transaction(tx);

        let resp = dummy_pool.add_unverified_transaction(Default::default(), utx);
        println!("{:?}", resp.wait());

        count = count + 1;

        thread::sleep(sleep_time);
    }
}
