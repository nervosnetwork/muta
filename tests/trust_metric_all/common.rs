use common_crypto::{
    Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey, Secp256k1Signature, Signature,
    ToPublicKey,
};
use protocol::{
    codec::ProtocolCodecSync,
    types::{Hash, RawTransaction, SignedTransaction, TransactionRequest},
    BytesMut,
};
use rand::{rngs::OsRng, RngCore};

use std::{convert::TryFrom, path::PathBuf};

const TX_CYCLE: u64 = 1;

pub fn tmp_dir() -> PathBuf {
    let mut tmp_dir = std::env::temp_dir();
    let sub_dir = {
        let mut random_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut random_bytes);
        Hash::digest(BytesMut::from(random_bytes.as_ref()).freeze()).as_hex()
    };

    tmp_dir.push(sub_dir + "/");
    tmp_dir
}

pub fn gen_signed_tx(
    priv_key: &Secp256k1PrivateKey,
    timeout: u64,
    valid: bool,
) -> SignedTransaction {
    let nonce = {
        let mut random_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut random_bytes);
        Hash::digest(BytesMut::from(random_bytes.as_ref()).freeze())
    };

    let request = TransactionRequest {
        service_name: "test".to_owned(),
        method:       "test".to_owned(),
        payload:      "test".to_owned(),
    };

    let raw = RawTransaction {
        chain_id: nonce.clone(),
        nonce,
        timeout,
        cycles_limit: TX_CYCLE,
        cycles_price: 1,
        request,
    };

    let raw_bytes = raw.encode_sync().expect("encode raw tx");
    let tx_hash = Hash::digest(raw_bytes);

    let signature = if valid {
        Secp256k1::sign_message(&tx_hash.as_bytes(), &priv_key.to_bytes()).expect("sign tx")
    } else {
        Secp256k1Signature::try_from([0u8; 64].as_ref()).expect("make invalid tx signature")
    };

    SignedTransaction {
        raw,
        tx_hash,
        pubkey: priv_key.pub_key().to_bytes(),
        signature: signature.to_bytes(),
    }
}
