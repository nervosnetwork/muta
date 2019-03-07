use database::memory::MemoryDB;
use transaction_pool::{order::FIFO, verifier::SECP256K1Verifier, TransactionPool};

fn main() {
    let memdb = MemoryDB::default();

    let order = FIFO::new();
    let verifier = SECP256K1Verifier::new();
    let _tx_pool = TransactionPool::new(memdb, order, verifier);

    println!("hello world");
}
