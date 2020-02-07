use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use cita_trie::MemoryDB;

use framework::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use framework::binding::state::{GeneralServiceState, MPTTrie};
use protocol::traits::{NoopDispatcher, ServiceSDK, Storage};
use protocol::types::{
    Address, Block, Hash, Metadata, Proof, Receipt, ServiceContext, ServiceContextParams,
    SignedTransaction, Validator, METADATA_KEY,
};
use protocol::{types::Bytes, ProtocolResult};

use crate::types::{SetAdminPayload, UpdateMetadataPayload};
use crate::{MetadataService, ADMIN_KEY};

#[test]
fn test_get_metadata() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());

    let init_metadata = mock_metadata_1();

    let service = new_metadata_service(init_metadata.clone(), caller);
    let metadata = service.get_metadata(context).unwrap();

    assert_eq!(metadata, init_metadata);
}

#[test]
fn test_update_metadata() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());

    let init_metadata = mock_metadata_1();
    let mut service = new_metadata_service(init_metadata.clone(), caller);

    let metadata = service.get_metadata(context.clone()).unwrap();
    assert_eq!(metadata, init_metadata);

    let update_metadata = mock_metadata_2();
    service
        .update_metadata(context.clone(), UpdateMetadataPayload {
            verifier_list:   update_metadata.verifier_list.clone(),
            interval:        update_metadata.interval,
            propose_ratio:   update_metadata.propose_ratio,
            prevote_ratio:   update_metadata.prevote_ratio,
            precommit_ratio: update_metadata.precommit_ratio,
        })
        .unwrap();

    let metadata = service.get_metadata(context).unwrap();
    assert_eq!(metadata, update_metadata);
}

#[test]
fn test_set_admin() {
    let admin_1: Address = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let admin_2: Address = Address::from_hex("f8389d774afdad8755ef8e629e5a154fddc6325a").unwrap();

    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let context = mock_context(cycles_limit, admin_1.clone());

    let init_metadata = mock_metadata_1();

    let mut service = new_metadata_service(init_metadata, admin_1.clone());
    let old_admin = service.get_admin(context.clone()).unwrap();
    assert_eq!(old_admin, admin_1);

    service
        .set_admin(context.clone(), SetAdminPayload {
            admin: admin_2.clone(),
        })
        .unwrap();
    let new_admin = service.get_admin(context).unwrap();
    assert_eq!(new_admin, admin_2);
}

fn new_metadata_service(
    metadata: Metadata,
    admin: Address,
) -> MetadataService<
    DefalutServiceSDK<
        GeneralServiceState<MemoryDB>,
        DefaultChainQuerier<MockStorage>,
        NoopDispatcher,
    >,
> {
    let chain_db = DefaultChainQuerier::new(Arc::new(MockStorage {}));
    let trie = MPTTrie::new(Arc::new(MemoryDB::new(false)));
    let state = GeneralServiceState::new(trie);

    let mut sdk = DefalutServiceSDK::new(
        Rc::new(RefCell::new(state)),
        Rc::new(chain_db),
        NoopDispatcher {},
    );

    sdk.set_value(METADATA_KEY.to_string(), metadata).unwrap();
    sdk.set_value(ADMIN_KEY.to_string(), admin).unwrap();

    MetadataService::new(sdk).unwrap()
}

fn mock_metadata_1() -> Metadata {
    Metadata {
        chain_id:        Hash::digest(Bytes::from("test")),
        common_ref:      "703873635a6b51513451".to_string(),
        timeout_gap:     20,
        cycles_limit:    99_999_999,
        cycles_price:    1,
        interval:        3000,
        verifier_list:   [Validator {
            address:        Address::from_hex("CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B").unwrap(),
            propose_weight: 1,
            vote_weight:    1,
        }]
        .to_vec(),
        propose_ratio:   10,
        prevote_ratio:   10,
        precommit_ratio: 10,
    }
}
fn mock_metadata_2() -> Metadata {
    Metadata {
        chain_id:        Hash::digest(Bytes::from("test")),
        common_ref:      "703873635a6b51513451".to_string(),
        timeout_gap:     20,
        cycles_limit:    99_999_999,
        cycles_price:    1,
        interval:        6000,
        verifier_list:   [
            Validator {
                address:        Address::from_hex("CAB8EEA4799C21379C20EF5BAA2CC8AFFFFFFFFF")
                    .unwrap(),
                propose_weight: 3,
                vote_weight:    13,
            },
            Validator {
                address:        Address::from_hex("FFFFFEA4799C21379C20EF5BAA2CC8AFFFFFFFFF")
                    .unwrap(),
                propose_weight: 3,
                vote_weight:    13,
            },
        ]
        .to_vec(),
        propose_ratio:   1,
        prevote_ratio:   1,
        precommit_ratio: 1,
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

    async fn update_muta_wal(&self, _info: Bytes) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn load_overlord_wal(&self) -> ProtocolResult<Bytes> {
        unimplemented!()
    }

    async fn load_muta_wal(&self) -> ProtocolResult<Bytes> {
        unimplemented!()
    }

    async fn update_exec_queue_wal(&self, _info: Bytes) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn load_exec_queue_wal(&self) -> ProtocolResult<Bytes> {
        unimplemented!()
    }

    async fn insert_wal_transactions(
        &self,
        _block_hash: Hash,
        _signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn get_wal_transactions(
        &self,
        _block_hash: Hash,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        unimplemented!()
    }
}
