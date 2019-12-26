extern crate test;

use test::Bencher;

use crate::fixed_codec::FixedCodec;
use crate::types;

use super::*;

macro_rules! test_eq {
    ($category: ident, $r#type: ident, $mock_func: ident $(, $arg: expr)*) => {
        let before_val = $mock_func($($arg),*);
        let rlp_bytes = before_val.encode_fixed().unwrap();
        let after_val: types::$category::$r#type = <_>::decode_fixed(rlp_bytes.clone()).unwrap();
        assert_eq!(before_val, after_val);
    };
}

#[test]
fn test_fixed_codec() {
    test_eq!(primitive, Hash, mock_hash);

    test_eq!(transaction, RawTransaction, mock_raw_tx);

    test_eq!(transaction, SignedTransaction, mock_sign_tx);

    test_eq!(epoch, Proof, mock_proof);
    test_eq!(epoch, EpochHeader, mock_epoch_header);
    test_eq!(epoch, Epoch, mock_epoch, 33);
    test_eq!(epoch, Pill, mock_pill, 22, 33);
    test_eq!(epoch, Validator, mock_validator);
    test_eq!(epoch, EpochId, mock_epoch_id);

    test_eq!(receipt, Receipt, mock_receipt);
    test_eq!(receipt, Receipt, mock_receipt);
    test_eq!(receipt, Receipt, mock_receipt);
    test_eq!(receipt, Receipt, mock_receipt);
}

#[test]
fn test_signed_tx_serialize_size() {
    let txs: Vec<Bytes> = (0..50_000)
        .map(|_| mock_sign_tx().encode_fixed().unwrap())
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
            signed_tx.encode_fixed().unwrap();
        });
    });
}

#[bench]
fn bench_signed_tx_deserialize(b: &mut Bencher) {
    let txs: Vec<Bytes> = (0..50_000)
        .map(|_| mock_sign_tx().encode_fixed().unwrap())
        .collect();

    b.iter(|| {
        txs.iter().for_each(|signed_tx| {
            SignedTransaction::decode_fixed(signed_tx.clone()).unwrap();
        });
    });
}

#[bench]
fn bench_epoch_serialize(b: &mut Bencher) {
    let epoch = mock_epoch(50_000);

    b.iter(|| {
        epoch.encode_fixed().unwrap();
    });
}

#[bench]
fn bench_epoch_deserialize(b: &mut Bencher) {
    let epoch = mock_epoch(50_000).encode_fixed().unwrap();

    b.iter(|| {
        Epoch::decode_fixed(epoch.clone()).unwrap();
    });
}
