use core_types::block::Block;
use core_types::transaction::SignedTransaction;

pub enum Message {
    Consensus(Vec<u8>),
    SignedTransaction(Box<SignedTransaction>),
    Block(Box<Block>),
}
