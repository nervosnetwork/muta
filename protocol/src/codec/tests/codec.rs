use std::convert::TryInto;

use test::Bencher;

use crate::codec::ProtocolCodecSync;
use crate::{codec, types};

use super::*;

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
    test!(primitive, AssetID, mock_asset_id);
    test!(primitive, AccountAddress, mock_account_address);
    test!(primitive, ContractAddress, mock_contract_address);
    test!(primitive, Asset, mock_asset);
    test!(primitive, Fee, mock_fee);

    test!(receipt, ReceiptResult, mock_result, ReceiptType::Transfer);
    test!(receipt, ReceiptResult, mock_result, ReceiptType::Approve);
    test!(receipt, ReceiptResult, mock_result, ReceiptType::Deploy);
    test!(receipt, ReceiptResult, mock_result, ReceiptType::Call);
    test!(receipt, ReceiptResult, mock_result, ReceiptType::Fail);
    test!(receipt, Receipt, mock_receipt, ReceiptType::Transfer);

    test!(transaction, TransactionAction, mock_action, AType::Transfer);
    test!(transaction, TransactionAction, mock_action, AType::Approve);
    test!(transaction, TransactionAction, mock_action, AType::Deploy);
    test!(transaction, TransactionAction, mock_action, AType::Call);
    test!(transaction, RawTransaction, mock_raw_tx, AType::Approve);
    test!(transaction, SignedTransaction, mock_sign_tx, AType::Deploy);

    test!(epoch, Validator, mock_validator);
    test!(epoch, Proof, mock_proof);
    test!(epoch, EpochId, mock_epoch_id);
    test!(epoch, EpochHeader, mock_epoch_header);
    test!(epoch, Epoch, mock_epoch, 100);
    test!(epoch, Pill, mock_pill, 100, 200);
}

#[bench]
fn bench_signed_tx_serialize(b: &mut Bencher) {
    let txs: Vec<SignedTransaction> = (0..50_000).map(|_| mock_sign_tx(AType::Transfer)).collect();
    b.iter(|| {
        txs.iter().for_each(|signed_tx| {
            signed_tx.encode_sync().unwrap();
        });
    });
}

#[bench]
fn bench_signed_tx_deserialize(b: &mut Bencher) {
    let txs: Vec<Bytes> = (0..50_000)
        .map(|_| mock_sign_tx(AType::Transfer).encode_sync().unwrap())
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
