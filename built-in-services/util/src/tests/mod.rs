use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use cita_trie::MemoryDB;
use rand::rngs::OsRng;

use async_trait::async_trait;
use common_crypto::{
    Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey, Signature, ToPublicKey,
};
use framework::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use framework::binding::state::{GeneralServiceState, MPTTrie};
use protocol::traits::{NoopDispatcher, Storage};
use protocol::types::{
    Address, Block, ChainSchema, Hash, Hex, Proof, Receipt, ServiceContext, ServiceContextParams,
    SignedTransaction,
};
use protocol::{types::Bytes, ProtocolResult};

use crate::types::{KeccakPayload, SigVerifyPayload};
use crate::UtilService;

#[test]
fn test_hash() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller);

    let service = new_util_service();

    let res = service
        .keccak256(context, KeccakPayload {
            hex_str: Hex::from_string("0x1234".to_string()).unwrap(),
        })
        .succeed_data;

    assert_eq!(
        res.result.as_hex(),
        "0x56570de287d73cd1cb6092bb8fdee6173974955fdef345ae579ee9f475ea7432".to_string()
    )
}

#[test]
fn test_verify() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller);

    let service = new_util_service();

    let priv_key = Secp256k1PrivateKey::generate(&mut OsRng);
    let pub_key = priv_key.pub_key();

    let mut input_pk: String = "0x".to_string();
    input_pk.push_str(hex::encode(pub_key.to_bytes()).as_str());

    let pub_key_data = Hex::from_string(input_pk).unwrap();
    let hash = Hash::from_hex("0x56570de287d73cd1cb6092bb8fdee6173974955fdef345ae579ee9f475ea7432")
        .unwrap();

    let sig = Secp256k1::sign_message(&hash.as_bytes(), &priv_key.to_bytes()).unwrap();
    let mut input_sig: String = "0x".to_string();
    input_sig.push_str(hex::encode(sig.to_bytes()).as_str());
    let sig_data = Hex::from_string(input_sig).unwrap();

    println!(
        "pubkey: {}\r\nsig: {}",
        pub_key_data.as_string(),
        sig_data.as_string()
    );

    let res = service
        .verify(context, SigVerifyPayload {
            hash,
            sig: sig_data,
            pub_key: pub_key_data,
        })
        .succeed_data;

    assert_eq!(res.is_ok, true)
}

fn new_util_service() -> UtilService<
    DefalutServiceSDK<
        GeneralServiceState<MemoryDB>,
        DefaultChainQuerier<MockStorage>,
        NoopDispatcher,
    >,
> {
    let chain_db = DefaultChainQuerier::new(Arc::new(MockStorage {}));
    let trie = MPTTrie::new(Arc::new(MemoryDB::new(false)));
    let state = GeneralServiceState::new(trie);

    let sdk = DefalutServiceSDK::new(
        Rc::new(RefCell::new(state)),
        Rc::new(chain_db),
        NoopDispatcher {},
    );

    UtilService::new(sdk)
}

fn mock_context(cycles_limit: u64, caller: Address) -> ServiceContext {
    let params = ServiceContextParams {
        tx_hash: None,
        nonce: None,
        cycles_limit,
        cycles_price: 1,
        cycles_used: Rc::new(RefCell::new(0)),
        caller,
        height: 1,
        timestamp: 0,
        service_name: "service_name".to_owned(),
        service_method: "service_method".to_owned(),
        service_payload: "service_payload".to_owned(),
        extra: None,
        events: Rc::new(RefCell::new(vec![])),
    };

    ServiceContext::new(params)
}

struct MockStorage;

#[async_trait]
impl Storage for MockStorage {
    async fn insert_transactions(&self, _: Vec<SignedTransaction>) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn insert_block(&self, _: Block) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn insert_receipts(&self, _: Vec<Receipt>) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn update_latest_proof(&self, _: Proof) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn get_transaction_by_hash(&self, _: Hash) -> ProtocolResult<SignedTransaction> {
        unimplemented!()
    }

    async fn get_transactions(&self, _: Vec<Hash>) -> ProtocolResult<Vec<SignedTransaction>> {
        unimplemented!()
    }

    async fn get_latest_block(&self) -> ProtocolResult<Block> {
        unimplemented!()
    }

    async fn get_block_by_height(&self, _: u64) -> ProtocolResult<Block> {
        unimplemented!()
    }

    async fn get_block_by_hash(&self, _: Hash) -> ProtocolResult<Block> {
        unimplemented!()
    }

    async fn get_receipt(&self, _: Hash) -> ProtocolResult<Receipt> {
        unimplemented!()
    }

    async fn get_receipts(&self, _: Vec<Hash>) -> ProtocolResult<Vec<Receipt>> {
        unimplemented!()
    }

    async fn get_latest_proof(&self) -> ProtocolResult<Proof> {
        unimplemented!()
    }

    async fn update_overlord_wal(&self, _info: Bytes) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn load_overlord_wal(&self) -> ProtocolResult<Bytes> {
        unimplemented!()
    }

    async fn insert_schema(&self, _cs: ChainSchema) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn get_schema(&self) -> ProtocolResult<ChainSchema> {
        unimplemented!()
    }
}
