use core_storage::storage::Storage;
use database::memory::MemoryDB;
use transaction_pool::{order::FIFO, verifier::SECP256K1Verifier, TransactionPool};

fn main() {
    let mut memdb = MemoryDB::default();
    let storage = Storage::new(&mut memdb);

    let order = FIFO::new();
    let verifier = SECP256K1Verifier::new();
    let tx_pool = TransactionPool::new(storage, order, verifier);

    println!("hello world");
}
