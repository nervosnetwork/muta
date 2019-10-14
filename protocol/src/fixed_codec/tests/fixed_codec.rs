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
        println!("before: {:?}", &before_val);
        println!("bytes: {:?}", &rlp_bytes);
        println!("after: {:?}", &after_val);

    };
}

#[test]
fn test_fixed_codec() {
    test!(primitive, Fee, mock_fee);
    test!(epoch, Proof, mock_proof);
    test!(epoch, EpochHeader, mock_epoch_header);
}

#[test]
fn test_fixed_fee() {
    let before_val = mock_fee();
    let rlp_bytes = before_val.encode_fixed().unwrap();
    let after_val: ProtocolResult<types::Fee> = <_>::decode_fixed(rlp_bytes.clone());
    println!("before: {:?}", &before_val);
    println!("bytes: {:?}", &rlp_bytes);
    println!("after: {:?}", &after_val);
}

#[test]
fn test_fixed_asset() {
    let before_val = mock_asset();
    let rlp_bytes = before_val.encode_fixed().unwrap();
    let after_val: ProtocolResult<types::Asset> = <_>::decode_fixed(rlp_bytes.clone());
    println!("before: {:?}", &before_val);
    println!("bytes: {:?}", &rlp_bytes);
    println!("after: {:?}", &after_val);
}