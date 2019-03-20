use log::info;
use logger;
use transaction_pool::{order::FIFO, verifier::SECP256K1Verifier, TransactionPool};

fn main() {
    logger::init(logger::Flag::Main);
    let order = FIFO::new();
    let verifier = SECP256K1Verifier::new();
    let _tx_pool = TransactionPool::new(order, verifier);

    info!("hello world");
}
