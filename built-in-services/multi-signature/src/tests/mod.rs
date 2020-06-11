mod curd_test;
mod recursion_test;

use std::cell::RefCell;
use std::convert::TryFrom;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use cita_trie::MemoryDB;
use rand::{random, thread_rng};

use common_crypto::{
    HashValue, PrivateKey, PublicKey, Secp256k1PrivateKey, Signature, ToPublicKey,
};
use framework::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use framework::binding::state::{GeneralServiceState, MPTTrie};
use protocol::traits::{Context, NoopDispatcher, Storage};
use protocol::types::{
    Address, Block, Hash, Proof, PubkeyWithSender, Receipt, ServiceContext, ServiceContextParams,
    SignedTransaction,
};
use protocol::{types::Bytes, ProtocolResult};

use crate::types::{MultiSigAccount, VerifySignaturePayload};
use crate::MultiSignatureService;

struct MockStorage;

#[async_trait]
impl Storage for MockStorage {
    async fn insert_transactions(
        &self,
        _ctx: Context,
        _: u64,
        _: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn insert_block(&self, _ctx: Context, _: Block) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn insert_receipts(&self, _ctx: Context, _: u64, _: Vec<Receipt>) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn update_latest_proof(&self, _ctx: Context, _: Proof) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn get_transaction_by_hash(
        &self,
        _ctx: Context,
        _: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>> {
        unimplemented!()
    }

    async fn get_transactions(
        &self,
        _ctx: Context,
        _: u64,
        _: Vec<Hash>,
    ) -> ProtocolResult<Vec<Option<SignedTransaction>>> {
        unimplemented!()
    }

    async fn get_latest_block(&self, _ctx: Context) -> ProtocolResult<Block> {
        unimplemented!()
    }

    async fn get_block(&self, _ctx: Context, _: u64) -> ProtocolResult<Option<Block>> {
        unimplemented!()
    }

    async fn get_receipt_by_hash(&self, _ctx: Context, _: Hash) -> ProtocolResult<Option<Receipt>> {
        unimplemented!()
    }

    async fn get_receipts(
        &self,
        _ctx: Context,
        _: u64,
        _: Vec<Hash>,
    ) -> ProtocolResult<Vec<Option<Receipt>>> {
        unimplemented!()
    }

    async fn get_latest_proof(&self, _ctx: Context) -> ProtocolResult<Proof> {
        unimplemented!()
    }

    async fn update_overlord_wal(&self, _ctx: Context, _info: Bytes) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn load_overlord_wal(&self, _ctx: Context) -> ProtocolResult<Bytes> {
        unimplemented!()
    }
}

fn new_multi_signature_service() -> MultiSignatureService<
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

    MultiSignatureService::new(sdk)
}

fn mock_context(cycles_limit: u64, caller: Address) -> ServiceContext {
    let params = ServiceContextParams {
        tx_hash: Some(mock_hash()),
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

fn mock_hash() -> Hash {
    Hash::digest(get_random_bytes(10))
}

fn get_random_bytes(len: usize) -> Bytes {
    let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
    Bytes::from(vec)
}

fn gen_one_keypair() -> (Bytes, Bytes) {
    let sk = Secp256k1PrivateKey::generate(&mut thread_rng());
    let pk = sk.pub_key();
    (sk.to_bytes(), pk.to_bytes())
}

fn gen_keypairs(num: usize) -> Vec<(Bytes, Bytes)> {
    (0..num).map(|_| gen_one_keypair()).collect::<Vec<_>>()
}

fn to_multi_sig_account(pk: Bytes) -> MultiSigAccount {
    MultiSigAccount {
        address: Address::from_pubkey_bytes(pk).unwrap(),
        weight:  1u8,
    }
}

fn gen_pubkey_with_sender_bytes(pk: Bytes) -> Bytes {
    Bytes::from(rlp::encode(&PubkeyWithSender {
        pubkey: pk,
        sender: None,
    }))
}

fn sign(privkey: &Bytes, hash: &Hash) -> Bytes {
    Secp256k1PrivateKey::try_from(privkey.as_ref())
        .unwrap()
        .sign_message(&HashValue::try_from(hash.as_bytes().as_ref()).unwrap())
        .to_bytes()
}

fn gen_single_witness(privkey: &Bytes, hash: &Hash) -> VerifySignaturePayload {
    let privkey = Secp256k1PrivateKey::try_from(privkey.as_ref()).unwrap();
    let pk = privkey.pub_key().to_bytes();
    let sig = privkey
        .sign_message(&HashValue::try_from(hash.as_bytes().as_ref()).unwrap())
        .to_bytes();

    VerifySignaturePayload {
        pubkeys:    vec![pk.clone()],
        signatures: vec![sig],
        sender:     Address::from_pubkey_bytes(pk).unwrap(),
    }
}
