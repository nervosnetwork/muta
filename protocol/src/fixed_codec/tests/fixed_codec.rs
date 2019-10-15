use crate::fixed_codec::{ProtocolFixedCodec};
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
    test_eq!(primitive, Fee, mock_fee);
    test_eq!(primitive, Hash, mock_hash);
    test_eq!(primitive, Asset, mock_asset);
    test_eq!(primitive, UserAddress, mock_account_address);
    test_eq!(primitive, ContractAddress, mock_contract_address);
    test_eq!(primitive, Account, mock_account_user);
    test_eq!(primitive, Account, mock_account_contract);

    test_eq!(transaction, RawTransaction, mock_raw_tx, AType::Transfer);
    test_eq!(transaction, RawTransaction, mock_raw_tx, AType::Approve);
    test_eq!(transaction, RawTransaction, mock_raw_tx, AType::Deploy);
    test_eq!(transaction, SignedTransaction, mock_sign_tx, AType::Transfer);
    test_eq!(transaction, SignedTransaction, mock_sign_tx, AType::Approve);
    test_eq!(transaction, SignedTransaction, mock_sign_tx, AType::Deploy);

    test_eq!(epoch, Proof, mock_proof);
    test_eq!(epoch, EpochHeader, mock_epoch_header);
    test_eq!(epoch, Epoch, mock_epoch, 33);
    test_eq!(epoch, Pill, mock_pill, 22, 33);
    test_eq!(epoch, Validator, mock_validator);
    test_eq!(epoch, EpochId, mock_epoch_id);

    test_eq!(receipt, Receipt, mock_receipt, ReceiptType::Transfer);
    test_eq!(receipt, Receipt, mock_receipt, ReceiptType::Approve);
    test_eq!(receipt, Receipt, mock_receipt, ReceiptType::Deploy);
    test_eq!(receipt, Receipt, mock_receipt, ReceiptType::Fail);

    test_eq!(genesis, Genesis, mock_genesis);
}
