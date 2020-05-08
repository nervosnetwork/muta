use super::node::consts;
use common_crypto::{
    Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey, Signature, ToPublicKey,
};
use protocol::{
    fixed_codec::FixedCodec,
    types::{
        Address, Hash, Hex, JsonString, RawTransaction, SignedTransaction, TransactionRequest,
        TypesError,
    },
    Bytes, BytesMut,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};

use std::{
    net::TcpListener,
    path::PathBuf,
    sync::atomic::{AtomicU16, Ordering},
};

use protocol::ProtocolResult;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct WitnessAdapter {
    pub pubkeys:        Vec<Hex>,
    pub signatures:     Vec<Hex>,
    pub signature_type: u8,
    pub sender:         Address,
}

impl WitnessAdapter {
    fn as_bytes(&self) -> ProtocolResult<Bytes> {
        match serde_json::to_vec(&self) {
            Ok(b) => Ok(Bytes::from(b)),
            Err(_) => Err(TypesError::InvalidWitness.into()),
        }
    }

    fn from_single_sig_hex(pub_key: String, sig: String) -> ProtocolResult<Self> {
        Ok(Self {
            pubkeys:        vec![Hex::from_string(pub_key)?],
            signatures:     vec![Hex::from_string(sig)?],
            signature_type: 0,
            sender:         Address::from_hex("0x0000000000000000000000000000000000000000")?,
        })
    }
}

static AVAILABLE_PORT: AtomicU16 = AtomicU16::new(2000);

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

    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = timeout;
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
        };

        let raw_bytes = raw.encode_fixed().expect("encode raw tx");
        let tx_hash = Hash::digest(raw_bytes);

        let sig = Secp256k1::sign_message(&tx_hash.as_bytes(), &pk.to_bytes()).expect("sign tx");

        let pubkey = Hex::from_bytes(pk.pub_key().to_bytes());
        let sig_hex = Hex::from_bytes(sig.to_bytes());

        let wit = WitnessAdapter::from_single_sig_hex(pubkey.as_string(), sig_hex.as_string())
            .expect("witness from_single_sig_hex");

        SignedTransaction {
            raw,
            tx_hash,
            witness: wit.as_bytes().expect("witness as_bytes"),
            sender: None,
        }
    }
}

pub fn stx_builder() -> SignedTransactionBuilder {
    SignedTransactionBuilder::default()
}

pub fn available_port_pair() -> (u16, u16) {
    (available_port(), available_port())
}

fn available_port() -> u16 {
    let is_available = |port| -> bool { TcpListener::bind(("127.0.0.1", port)).is_ok() };

    loop {
        let port = AVAILABLE_PORT.fetch_add(1, Ordering::SeqCst);
        if is_available(port) {
            return port;
        }
    }
}
