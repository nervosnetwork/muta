use common_crypto::{
    Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey, Signature, ToPublicKey,
};
use protocol::fixed_codec::FixedCodec;
use protocol::types::{
    Address, Hash, JsonString, RawTransaction, SignedTransaction, TransactionRequest,
};
use protocol::{Bytes, BytesMut};
use rand::{rngs::OsRng, RngCore};

use crate::common::node::consts;

pub struct SignedTransactionBuilder {
    chain_id:     Hash,
    timeout:      u64,
    cycles_limit: u64,
    payload:      JsonString,
}

impl Default for SignedTransactionBuilder {
    fn default() -> Self {
        let chain_id = Hash::from_hex(consts::CHAIN_ID).expect("chain id");
        let timeout = 19;
        let cycles_limit = 314_159;
        let payload = "test".to_owned();

        SignedTransactionBuilder {
            chain_id,
            timeout,
            cycles_limit,
            payload,
        }
    }
}

impl SignedTransactionBuilder {
    pub fn chain_id(mut self, chain_id_bytes: Bytes) -> Self {
        self.chain_id = Hash::digest(chain_id_bytes);
        self
    }

    pub fn cycles_limit(mut self, cycles_limit: u64) -> Self {
        self.cycles_limit = cycles_limit;
        self
    }

    pub fn payload(mut self, payload: JsonString) -> Self {
        self.payload = payload;
        self
    }

    pub fn build(self, pk: &Secp256k1PrivateKey) -> SignedTransaction {
        let nonce = {
            let mut random_bytes = [0u8; 32];
            OsRng.fill_bytes(&mut random_bytes);
            Hash::digest(BytesMut::from(random_bytes.as_ref()).freeze())
        };

        let request = TransactionRequest {
            service_name: "metadata".to_owned(),
            method:       "get_metadata".to_owned(),
            payload:      self.payload,
        };

        let raw = RawTransaction {
            chain_id: self.chain_id,
            nonce,
            timeout: self.timeout,
            cycles_limit: self.cycles_limit,
            cycles_price: 1,
            request,
            sender: Address::from_pubkey_bytes(pk.pub_key().to_bytes()).unwrap(),
        };

        let raw_bytes = raw.encode_fixed().expect("encode raw tx");
        let tx_hash = Hash::digest(raw_bytes);

        let sig = Secp256k1::sign_message(&tx_hash.as_bytes(), &pk.to_bytes()).expect("sign tx");

        SignedTransaction {
            raw,
            tx_hash,
            pubkey: Bytes::from(rlp::encode_list::<Vec<u8>, _>(&[pk
                .pub_key()
                .to_bytes()
                .to_vec()])),
            signature: Bytes::from(rlp::encode_list::<Vec<u8>, _>(&[sig.to_bytes().to_vec()])),
        }
    }
}

pub fn stx_builder() -> SignedTransactionBuilder {
    SignedTransactionBuilder::default()
}
