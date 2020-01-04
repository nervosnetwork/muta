extern crate test;

use std::convert::TryInto;

use bytes::Bytes;
use test::Bencher;

use crate::codec::ProtocolCodecSync;
use crate::types::epoch::Epoch;
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

    test!(epoch, Validator, mock_validator);
    test!(epoch, Proof, mock_proof);
    test!(epoch, EpochHeader, mock_epoch_header);
    test!(epoch, Epoch, mock_epoch, 100);
    test!(epoch, Pill, mock_pill, 100, 200);
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
fn bench_epoch_serialize(b: &mut Bencher) {
    let epoch = mock_epoch(50_000);

    b.iter(|| {
        epoch.encode_sync().unwrap();
    });
}

#[bench]
fn bench_epoch_try_into(b: &mut Bencher) {
    let epoch = mock_epoch(50_000).encode_sync().unwrap();

    b.iter(|| {
        Epoch::decode_sync(epoch.clone()).unwrap();
    });
}