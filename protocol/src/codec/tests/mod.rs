extern crate test;

use std::convert::TryInto;

use bytes::Bytes;
use test::Bencher;

use crate::codec::ProtocolCodecSync;
use crate::types::block::Block;
use crate::types::transaction::SignedTransaction;
use crate::{codec, types};

use crate::fixed_codec::tests::*;

macro_rules! test {
    ($mod: ident, $r#type: ident, $mock_func: ident $(, $arg: expr)*) => {
        {
            let before_val = $mock_func($($arg),*);
            let codec_val: codec::$mod::$r#type = before_val.into();
            let after_val: types::$mod::$r#type = codec_val.try_into().unwrap();
            after_val
        }
    };
}

#[test]
fn test_codec() {
    test!(primitive, Balance, mock_balance);
    test!(primitive, Hash, mock_hash);
    test!(primitive, MerkleRoot, mock_merkle_root);

    test!(receipt, Receipt, mock_receipt);

    test!(transaction, TransactionRequest, mock_transaction_request);
    test!(transaction, RawTransaction, mock_raw_tx);
    test!(transaction, SignedTransaction, mock_sign_tx);

    test!(block, Validator, mock_validator);
    test!(block, Proof, mock_proof);
    test!(block, BlockHeader, mock_block_header);
    test!(block, Block, mock_block, 100);
    test!(block, Pill, mock_pill, 100, 200);
}

#[test]
fn test_signed_tx_serialize_size() {
    let txs: Vec<Bytes> = (0..50_000)
        .map(|_| mock_sign_tx().encode_sync().unwrap())
        .collect();
    let size = &txs.iter().fold(0, |acc, x| acc + x.len());
    println!("1 tx size {:?}", txs[1].len());
    println!("50_000 tx size {:?}", size);
}

#[bench]
fn bench_signed_tx_serialize(b: &mut Bencher) {
    let txs: Vec<SignedTransaction> = (0..50_000).map(|_| mock_sign_tx()).collect();
    b.iter(|| {
        txs.iter().for_each(|signed_tx| {
            signed_tx.encode_sync().unwrap();
        });
    });
}

#[bench]
fn bench_signed_tx_deserialize(b: &mut Bencher) {
    let txs: Vec<Bytes> = (0..50_000)
        .map(|_| mock_sign_tx().encode_sync().unwrap())
        .collect();

    b.iter(|| {
        txs.iter().for_each(|signed_tx| {
            SignedTransaction::decode_sync(signed_tx.clone()).unwrap();
        });
    });
}

#[bench]
fn bench_block_serialize(b: &mut Bencher) {
    let block = mock_block(50_000);

    b.iter(|| {
        block.encode_sync().unwrap();
    });
}

#[bench]
fn bench_block_try_into(b: &mut Bencher) {
    let block = mock_block(50_000).encode_sync().unwrap();

    b.iter(|| {
        Block::decode_sync(block.clone()).unwrap();
    });
}
