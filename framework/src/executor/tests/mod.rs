extern crate test;

mod service_call_service;
mod test_service;

use std::sync::Arc;

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use cita_trie::MemoryDB;
use test::Bencher;

use asset::types::{Asset, GetBalanceResponse};
use asset::AssetService;
use metadata::MetadataService;
use protocol::traits::{
    Context, Executor, ExecutorParams, Service, ServiceMapping, ServiceSDK, Storage,
};
use protocol::types::{
    Address, Block, Genesis, Hash, Proof, RawTransaction, Receipt, SignedTransaction,
    TransactionRequest,
};
use protocol::ProtocolResult;

use crate::executor::ServiceExecutor;
use test_service::TestService;

#[test]
fn test_create_genesis() {
    let toml_str = include_str!("./genesis_services.toml");
    let genesis: Genesis = toml::from_str(toml_str).unwrap();

    let db = Arc::new(MemoryDB::new(false));

    let root = ServiceExecutor::create_genesis(
        genesis.services,
        Arc::clone(&db),
        Arc::new(MockStorage {}),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();

    let executor = ServiceExecutor::with_root(
        root.clone(),
        Arc::clone(&db),
        Arc::new(MockStorage {}),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();
    let params = ExecutorParams {
        state_root:   root,
        height:       1,
        timestamp:    0,
        cycles_limit: std::u64::MAX,
    };
    let caller = Address::from_hex("0xf8389d774afdad8755ef8e629e5a154fddc6325a").unwrap();
    let request = TransactionRequest {
        service_name: "asset".to_owned(),
        method:       "get_balance".to_owned(),
        payload:
            r#"{"asset_id": "0xf56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c", "user": "0xf8389d774afdad8755ef8e629e5a154fddc6325a"}"#
                .to_owned(),
    };
    let res = executor.read(&params, &caller, 1, &request).unwrap();
    let resp: GetBalanceResponse = serde_json::from_str(&res.succeed_data).unwrap();

    assert_eq!(resp.balance, 320_000_011);
}

#[test]
fn test_exec() {
    let toml_str = include_str!("./genesis_services.toml");
    let genesis: Genesis = toml::from_str(toml_str).unwrap();

    let db = Arc::new(MemoryDB::new(false));

    let root = ServiceExecutor::create_genesis(
        genesis.services,
        Arc::clone(&db),
        Arc::new(MockStorage {}),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();

    let mut executor = ServiceExecutor::with_root(
        root.clone(),
        Arc::clone(&db),
        Arc::new(MockStorage {}),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();

    let params = ExecutorParams {
        state_root:   root,
        height:       1,
        timestamp:    0,
        cycles_limit: std::u64::MAX,
    };

    let stx = mock_signed_tx();
    let txs = vec![stx];
    let executor_resp = executor.exec(Context::new(), &params, &txs).unwrap();
    let receipt = &executor_resp.receipts[0];

    assert_eq!(receipt.response.response.code, 0);
    let asset: Asset = serde_json::from_str(&receipt.response.response.succeed_data).unwrap();
    assert_eq!(asset.name, "MutaToken2");
    assert_eq!(asset.symbol, "MT2");
    assert_eq!(asset.supply, 320_000_011);
}

#[test]
fn test_tx_hook() {
    let toml_str = include_str!("./genesis_services.toml");
    let genesis: Genesis = toml::from_str(toml_str).unwrap();

    let db = Arc::new(MemoryDB::new(false));

    let root = ServiceExecutor::create_genesis(
        genesis.services,
        Arc::clone(&db),
        Arc::new(MockStorage {}),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();

    let mut executor = ServiceExecutor::with_root(
        root.clone(),
        Arc::clone(&db),
        Arc::new(MockStorage {}),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();

    let params = ExecutorParams {
        state_root:   root,
        height:       1,
        timestamp:    0,
        cycles_limit: std::u64::MAX,
    };

    // no tx hook
    let mut stx = mock_signed_tx();
    stx.raw.request.service_name = "test".to_owned();
    stx.raw.request.method = "test_write".to_owned();
    stx.raw.request.payload = r#"{
        "key": "foo",
        "value": "bar",
        "extra": ""
    }"#
    .to_owned();
    let txs = vec![stx.clone()];
    let executor_resp = executor.exec(Context::new(), &params, &txs).unwrap();
    let receipt = &executor_resp.receipts[0];
    assert_eq!(receipt.response.response.code, 0);
    assert_eq!(receipt.events.len(), 0);

    // tx hook
    stx.raw.request.payload = r#"{
        "key": "foo",
        "value": "bar",
        "extra": "test_hook_before; test_hook_after"
    }"#
    .to_owned();
    let txs = vec![stx.clone()];
    let executor_resp = executor.exec(Context::new(), &params, &txs).unwrap();
    let receipt = &executor_resp.receipts[0];
    assert_eq!(receipt.response.response.code, 0);
    assert_eq!(receipt.events.len(), 2);
    assert_eq!(&receipt.events[0].data, "test_tx_hook_before invoked");
    assert_eq!(&receipt.events[1].data, "test_tx_hook_after invoked");

    // test_service_call_invoke_hook_only_once
    stx.raw.request.method = "test_service_call_invoke_hook_only_once".to_owned();
    stx.raw.request.payload = r#"{
        "key": "foo",
        "value": "bar",
        "extra": "test_hook_before; test_hook_after"
    }"#
    .to_owned();
    let txs = vec![stx];
    let executor_resp = executor.exec(Context::new(), &params, &txs).unwrap();
    let receipt = &executor_resp.receipts[0];
    assert_eq!(receipt.response.response.code, 0);
    assert_eq!(receipt.events.len(), 2);
    assert_eq!(&receipt.events[0].data, "test_tx_hook_before invoked");
    assert_eq!(&receipt.events[1].data, "test_tx_hook_after invoked");
}

#[bench]
fn bench_execute(b: &mut Bencher) {
    let toml_str = include_str!("./genesis_services.toml");
    let genesis: Genesis = toml::from_str(toml_str).unwrap();

    let db = Arc::new(MemoryDB::new(false));

    let root = ServiceExecutor::create_genesis(
        genesis.services,
        Arc::clone(&db),
        Arc::new(MockStorage {}),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();

    let mut executor = ServiceExecutor::with_root(
        root.clone(),
        Arc::clone(&db),
        Arc::new(MockStorage {}),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();

    let txs: Vec<SignedTransaction> = (0..1000).map(|_| mock_signed_tx()).collect();

    b.iter(|| {
        let params = ExecutorParams {
            state_root:   root.clone(),
            height:       1,
            timestamp:    0,
            cycles_limit: std::u64::MAX,
        };
        let txs = txs.clone();
        executor.exec(Context::new(), &params, &txs).unwrap();
    });
}

fn mock_signed_tx() -> SignedTransaction {
    let raw = RawTransaction {
        chain_id:     Hash::from_empty(),
        nonce:        Hash::from_empty(),
        timeout:      0,
        cycles_price: 1,
        cycles_limit: std::u64::MAX,
        request:      TransactionRequest {
            service_name: "asset".to_owned(),
            method:       "create_asset".to_owned(),
            payload:      r#"{ "name": "MutaToken2", "symbol": "MT2", "supply": 320000011 }"#
                .to_owned(),
        },
    };

    SignedTransaction {
        raw,
        tx_hash: Hash::from_empty(),
        pubkey: Bytes::from(
            hex::decode("031288a6788678c25952eba8693b2f278f66e2187004b64ac09416d07f83f96d5b")
                .unwrap(),
        ),
        signature: BytesMut::from("").freeze(),
    }
}

struct MockServiceMapping;

impl ServiceMapping for MockServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK>(
        &self,
        name: &str,
        sdk: SDK,
    ) -> ProtocolResult<Box<dyn Service>> {
        let service = match name {
            "asset" => Box::new(AssetService::new(sdk)) as Box<dyn Service>,
            "metadata" => Box::new(MetadataService::new(sdk)) as Box<dyn Service>,
            "test" => Box::new(TestService::new(sdk)) as Box<dyn Service>,
            _ => panic!("not found service"),
        };

        Ok(service)
    }

    fn list_service_name(&self) -> Vec<String> {
        vec!["asset".to_owned(), "metadata".to_owned(), "test".to_owned()]
    }
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
