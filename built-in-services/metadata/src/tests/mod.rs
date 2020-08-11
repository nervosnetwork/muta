use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use cita_trie::MemoryDB;

use framework::binding::sdk::{DefaultChainQuerier, DefaultServiceSDK};
use framework::binding::state::{GeneralServiceState, MPTTrie};
use protocol::traits::{Context, ServiceSDK, Storage};
use protocol::types::{
    Address, Block, Hash, Hex, Metadata, Proof, Receipt, ServiceContext, ServiceContextParams,
    SignedTransaction, ValidatorExtend, METADATA_KEY,
};
use protocol::{types::Bytes, ProtocolResult};

use crate::MetadataService;

#[test]
fn test_get_metadata() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_str("muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705").unwrap();
    let context = mock_context(cycles_limit, caller);

    let init_metadata = mock_metadata();

    let service = new_metadata_service_with_metadata(init_metadata.clone());
    let metadata = service.get_metadata(context).succeed_data;

    assert_eq!(metadata, init_metadata);
}

fn new_metadata_service_with_metadata(
    metadata: Metadata,
) -> MetadataService<
    DefaultServiceSDK<GeneralServiceState<MemoryDB>, DefaultChainQuerier<MockStorage>>,
> {
    let chain_db = DefaultChainQuerier::new(Arc::new(MockStorage {}));
    let trie = MPTTrie::new(Arc::new(MemoryDB::new(false)));
    let state = GeneralServiceState::new(trie);

    let mut sdk = DefaultServiceSDK::new(Rc::new(RefCell::new(state)), Rc::new(chain_db));

    sdk.set_value(METADATA_KEY.to_string(), metadata);

    MetadataService::new(sdk)
}

fn mock_metadata() -> Metadata {
    Metadata {
        chain_id:        Hash::digest(Bytes::from("test")),
        bech32_address_hrp: "muta".to_owned(),
        common_ref:      Hex::from_string("0x703873635a6b51513451".to_string()).unwrap(),
        timeout_gap:     20,
        cycles_limit:    99_999_999,
        cycles_price:    1,
       interval:        3000,
       verifier_list:   [ValidatorExtend {
           bls_pub_key: Hex::from_string("0x04102947214862a503c73904deb5818298a186d68c7907bb609583192a7de6331493835e5b8281f4d9ee705537c0e765580e06f86ddce5867812fceb42eecefd209f0eddd0389d6b7b0100f00fb119ef9ab23826c6ea09aadcc76fa6cea6a32724".to_owned()).unwrap(),
           pub_key:     Hex::from_string("0x02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60".to_owned()).unwrap(),
           address:     Address::from_str("muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705").unwrap(),
           propose_weight: 1,
           vote_weight:    1,
       }]
        .to_vec(),
        propose_ratio:   10,
        prevote_ratio:   10,
        precommit_ratio: 10,
        brake_ratio:     7,
        tx_num_limit: 20000,
        max_tx_size: 1_073_741_824,
    }
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
