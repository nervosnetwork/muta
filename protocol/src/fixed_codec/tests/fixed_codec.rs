use bytes::Bytes;

use crate::fixed_codec::{ProtocolFixedCodec};
use crate::ProtocolResult;
use crate::types;

use super::*;

macro_rules! test {
    ($category: ident, $r#type: ident, $mock_func: ident $(, $arg: expr)*) => {
        let before_val = $mock_func($($arg),*);
        let rlp_bytes = before_val.encode_fixed().unwrap();
        let after_val: types::$category::$r#type = <_>::decode_fixed(rlp_bytes.clone()).unwrap();
        // println!("before: {:?}", &before_val);
        // println!("bytes: {:?}", &rlp_bytes);
        // println!("after: {:?}", &after_val);
        // assert_eq!(before_val, after_val);
    };
}

#[test]
fn test_fixed_codec() {
    test!(primitive, Fee, mock_fee);
    test!(primitive, Hash, mock_hash);
    test!(primitive, Asset, mock_asset);
    test!(primitive, Account, mock_account);

    test!(transaction, RawTransaction, mock_raw_tx, AType::Transfer);
    test!(transaction, RawTransaction, mock_raw_tx, AType::Approve);
    test!(transaction, RawTransaction, mock_raw_tx, AType::Deploy);
    test!(transaction, SignedTransaction, mock_sign_tx, AType::Transfer);
    test!(transaction, SignedTransaction, mock_sign_tx, AType::Approve);
    test!(transaction, SignedTransaction, mock_sign_tx, AType::Deploy);

    test!(epoch, Proof, mock_proof);
    test!(epoch, EpochHeader, mock_epoch_header);
    test!(epoch, Epoch, mock_epoch, 33);
    test!(epoch, Pill, mock_pill, 22, 33);
    test!(epoch, Validator, mock_validator);

    test!(receipt, Receipt, mock_receipt, ReceiptType::Transfer);
    test!(receipt, Receipt, mock_receipt, ReceiptType::Approve);
    test!(receipt, Receipt, mock_receipt, ReceiptType::Deploy);
    test!(receipt, Receipt, mock_receipt, ReceiptType::Fail);
}
