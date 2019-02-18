use futures::future::Future;
use std::default::Default;
use std::{thread, time};
use umaru::{client::rpc::Client, proto::blockchain, service::PoolService};

fn main() {
    let client = Client::new().unwrap();

    let mut count = 1;
    let sleep_time = time::Duration::from_secs(1);

    loop {
        let mut utx = blockchain::UnverifiedTransaction::new();
        let mut tx = blockchain::Transaction::new();
        tx.set_nonce(format!("{}", count));
        tx.set_quota(111);
        tx.set_valid_until_block(222);
        tx.set_data(format!("data {}", count).as_bytes().to_vec());
        utx.set_transaction(tx);

        let resp = client
            .pool
            .add_unverified_transaction(Default::default(), utx);
        println!("{:?}", resp.wait());

        count = count + 1;

        thread::sleep(sleep_time);
    }
}
