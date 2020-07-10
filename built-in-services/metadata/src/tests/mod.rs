use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use cita_trie::MemoryDB;

use framework::binding::sdk::{DefaultChainQuerier, DefaultServiceSDK};
use framework::binding::state::{GeneralServiceState, MPTTrie};
use protocol::traits::{Context, NoopDispatcher, ServiceSDK, Storage};
use protocol::types::{
    Address, Block, Hash, Hex, Metadata, Proof, Receipt, ServiceContext, ServiceContextParams,
    SignedTransaction, ValidatorExtend, METADATA_KEY,
};
use protocol::{types::Bytes, ProtocolResult};

use crate::MetadataService;

#[test]
fn test_get_metadata() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller);

    let init_metadata = mock_metadata();

    let service = new_metadata_service_with_metadata(init_metadata.clone());
    let metadata = service.get_metadata(context).succeed_data;

    assert_eq!(metadata, init_metadata);
}

fn new_metadata_service_with_metadata(
    metadata: Metadata,
) -> MetadataService<
    DefaultServiceSDK<
        GeneralServiceState<MemoryDB>,
        DefaultChainQuerier<MockStorage>,
        NoopDispatcher,
    >,
> {
    let chain_db = DefaultChainQuerier::new(Arc::new(MockStorage {}));
    let trie = MPTTrie::new(Arc::new(MemoryDB::new(false)));
    let state = GeneralServiceState::new(trie);

    let mut sdk = DefaultServiceSDK::new(
        Rc::new(RefCell::new(state)),
        Rc::new(chain_db),
        NoopDispatcher {},
    );

    sdk.set_value(METADATA_KEY.to_string(), metadata);

    MetadataService::new(sdk)
}

fn mock_metadata() -> Metadata {
    Metadata {
        chain_id:        Hash::digest(Bytes::from("test")),
        common_ref:      Hex::from_string("0x703873635a6b51513451".to_string()).unwrap(),
        timeout_gap:     20,
        cycles_limit:    99_999_999,
        cycles_price:    1,
        interval:        3000,
        verifier_list:   [ValidatorExtend {
            bls_pub_key: Hex::from_string("0x04188ef9488c19458a963cc57b567adde7db8f8b6bec392d5cb7b67b0abc1ed6cd966edc451f6ac2ef38079460eb965e890d1f576e4039a20467820237cda753f07a8b8febae1ec052190973a1bcf00690ea8fc0168b3fbbccd1c4e402eda5ef22".to_owned()).unwrap(),
            peer_id:        Bytes::from(hex::decode("0405e7689f808af9fea532548b1b90d1fac48112b4a6a83bc331629df70647a84cf8a0dbc73352ab78664a15f57caaef860f3a6c6ceb128f6ec01a86ac96c8b7f2ba3be79387faf69c7f3bd112f0ddf3c6225a7fe23ec0c680cf93580716dd6fe4".to_string()).unwrap()),
            address: Address::from_hex("0xCAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B").unwrap(),
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
