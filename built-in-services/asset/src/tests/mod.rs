use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use cita_trie::MemoryDB;

use framework::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use framework::binding::state::{GeneralServiceState, MPTTrie};
use protocol::traits::{NoopDispatcher, Storage};
use protocol::types::{
    Address, Block, Hash, Proof, Receipt, ServiceContext, ServiceContextParams, SignedTransaction,
};
use protocol::{types::Bytes, ProtocolResult};

use crate::types::{
    ApprovePayload, CreateAssetPayload, GetAllowancePayload, GetAssetPayload, GetBalancePayload,
    TransferFromPayload, TransferPayload,
};
use crate::AssetService;

#[test]
fn test_create_asset() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());

    let mut service = new_asset_service();

    let supply = 1024 * 1024;
    // test create_asset
    let asset = service
        .create_asset(context.clone(), CreateAssetPayload {
            name: "test".to_owned(),
            symbol: "test".to_owned(),
            supply,
        })
        .unwrap();

    let new_asset = service
        .get_asset(context.clone(), GetAssetPayload {
            id: asset.id.clone(),
        })
        .unwrap();
    assert_eq!(asset, new_asset);

    let balance_res = service
        .get_balance(context, GetBalancePayload {
            asset_id: asset.id.clone(),
            user:     caller,
        })
        .unwrap();
    assert_eq!(balance_res.balance, supply);
    assert_eq!(balance_res.asset_id, asset.id);
}

#[test]
fn test_transfer() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());

    let mut service = new_asset_service();

    let supply = 1024 * 1024;
    // test create_asset
    let asset = service
        .create_asset(context.clone(), CreateAssetPayload {
            name: "test".to_owned(),
            symbol: "test".to_owned(),
            supply,
        })
        .unwrap();

    let to_address = Address::from_hex("0x666cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    service
        .transfer(context.clone(), TransferPayload {
            asset_id: asset.id.clone(),
            to:       to_address.clone(),
            value:    1024,
        })
        .unwrap();

    let balance_res = service
        .get_balance(context, GetBalancePayload {
            asset_id: asset.id.clone(),
            user:     caller,
        })
        .unwrap();
    assert_eq!(balance_res.balance, supply - 1024);

    let context = mock_context(cycles_limit, to_address.clone());
    let balance_res = service
        .get_balance(context, GetBalancePayload {
            asset_id: asset.id,
            user:     to_address,
        })
        .unwrap();
    assert_eq!(balance_res.balance, 1024);
}

#[test]
fn test_approve() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());

    let mut service = new_asset_service();

    let supply = 1024 * 1024;
    let asset = service
        .create_asset(context.clone(), CreateAssetPayload {
            name: "test".to_owned(),
            symbol: "test".to_owned(),
            supply,
        })
        .unwrap();

    let to_address = Address::from_hex("0x666cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    service
        .approve(context.clone(), ApprovePayload {
            asset_id: asset.id.clone(),
            to:       to_address.clone(),
            value:    1024,
        })
        .unwrap();

    let allowance_res = service
        .get_allowance(context, GetAllowancePayload {
            asset_id: asset.id.clone(),
            grantor:  caller,
            grantee:  to_address.clone(),
        })
        .unwrap();
    assert_eq!(allowance_res.asset_id, asset.id);
    assert_eq!(allowance_res.grantee, to_address);
    assert_eq!(allowance_res.value, 1024);
}

#[test]
fn test_transfer_from() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());

    let mut service = new_asset_service();

    let supply = 1024 * 1024;
    let asset = service
        .create_asset(context.clone(), CreateAssetPayload {
            name: "test".to_owned(),
            symbol: "test".to_owned(),
            supply,
        })
        .unwrap();

    let to_address = Address::from_hex("0x666cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    service
        .approve(context.clone(), ApprovePayload {
            asset_id: asset.id.clone(),
            to:       to_address.clone(),
            value:    1024,
        })
        .unwrap();

    let to_context = mock_context(cycles_limit, to_address.clone());

    service
        .transfer_from(to_context.clone(), TransferFromPayload {
            asset_id:  asset.id.clone(),
            sender:    caller.clone(),
            recipient: to_address.clone(),
            value:     24,
        })
        .unwrap();

    let allowance_res = service
        .get_allowance(context.clone(), GetAllowancePayload {
            asset_id: asset.id.clone(),
            grantor:  caller.clone(),
            grantee:  to_address.clone(),
        })
        .unwrap();
    assert_eq!(allowance_res.asset_id, asset.id.clone());
    assert_eq!(allowance_res.grantee, to_address.clone());
    assert_eq!(allowance_res.value, 1000);

    let balance_res = service
        .get_balance(context, GetBalancePayload {
            asset_id: asset.id.clone(),
            user:     caller,
        })
        .unwrap();
    assert_eq!(balance_res.balance, supply - 24);

    let balance_res = service
        .get_balance(to_context, GetBalancePayload {
            asset_id: asset.id,
            user:     to_address,
        })
        .unwrap();
    assert_eq!(balance_res.balance, 24);
}

fn new_asset_service() -> AssetService<
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

    AssetService::new(sdk).unwrap()
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
