use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use cita_trie::MemoryDB;

use asset::types::{Asset, CreateAssetPayload};
use asset::{AssetService, ServiceError};
use binding_macro::{cycles, service};
use metadata::MetadataService;

use protocol::traits::{Executor, ExecutorParams, Service, ServiceMapping, ServiceSDK};
use protocol::types::{
    Genesis, Hash, RawTransaction, ServiceContext, SignedTransaction, TransactionRequest,
};
use protocol::ProtocolResult;

use crate::executor::tests::MockStorage;
use crate::executor::ServiceExecutor;

#[test]
fn test_service_call_service() {
    let memdb = Arc::new(MemoryDB::new(false));
    let arcs = Arc::new(MockStorage {});

    let toml_str = include_str!("./genesis_services.toml");
    let genesis: Genesis = toml::from_str(toml_str).unwrap();

    let root = ServiceExecutor::create_genesis(
        genesis.services,
        Arc::clone(&memdb),
        Arc::new(MockStorage {}),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();

    let mut executor = ServiceExecutor::with_root(
        root.clone(),
        Arc::clone(&memdb),
        Arc::clone(&arcs),
        Arc::new(MockServiceMapping {}),
    )
    .unwrap();

    let params = ExecutorParams {
        state_root:   root,
        height:       1,
        timestamp:    0,
        cycles_limit: std::u64::MAX,
    };

    let raw = RawTransaction {
        chain_id:     Hash::from_empty(),
        nonce:        Hash::from_empty(),
        timeout:      0,
        cycles_price: 1,
        cycles_limit: 60_000,
        request:      TransactionRequest {
            service_name: "mock".to_owned(),
            method:       "call_asset".to_owned(),
            payload:      r#"{ "name": "TestCallAsset", "symbol": "TCA", "supply": 320000011 }"#
                .to_owned(),
        },
    };
    let stx = SignedTransaction {
        raw,
        tx_hash: Hash::from_empty(),
        pubkey: Bytes::from(
            hex::decode("031288a6788678c25952eba8693b2f278f66e2187004b64ac09416d07f83f96d5b")
                .unwrap(),
        ),
        signature: BytesMut::from("").freeze(),
    };

    let txs = vec![stx];
    let executor_resp = executor.exec(&params, &txs).unwrap();
    let receipt = &executor_resp.receipts[0];
    let event = &receipt.events[0];

    assert_eq!(50_000, receipt.cycles_used);
    assert_eq!(
        ("mock", "call create asset succeed"),
        (event.service.as_str(), event.data.as_str())
    );

    assert_eq!(receipt.response.is_error, false);
    let asset: Asset = serde_json::from_str(&receipt.response.ret).unwrap();
    assert_eq!(asset.name, "TestCallAsset");
    assert_eq!(asset.symbol, "TCA");
    assert_eq!(asset.supply, 320_000_011);
}

pub struct MockService<SDK> {
    sdk: SDK,
}

#[service]
impl<SDK: ServiceSDK> MockService<SDK> {
    pub fn new(sdk: SDK) -> ProtocolResult<Self> {
        Ok(Self { sdk })
    }

    #[cycles(290_00)]
    #[write]
    fn call_asset(
        &mut self,
        ctx: ServiceContext,
        payload: CreateAssetPayload,
    ) -> ProtocolResult<Asset> {
        let payload_str = serde_json::to_string(&payload).map_err(ServiceError::JsonParse)?;

        let ret = self
            .sdk
            .write(&ctx, None, "asset", "create_asset", &payload_str)?;

        let asset: Asset = serde_json::from_str(&ret).unwrap();

        ctx.emit_event("call create asset succeed".to_owned())
            .unwrap();
        Ok(asset)
    }
}

pub struct MockServiceMapping;

impl ServiceMapping for MockServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK>(
        &self,
        name: &str,
        sdk: SDK,
    ) -> ProtocolResult<Box<dyn Service>> {
        let service = match name {
            "mock" => Box::new(MockService::new(sdk)?) as Box<dyn Service>,
            "asset" => Box::new(AssetService::new(sdk)?) as Box<dyn Service>,
            "metadata" => Box::new(MetadataService::new(sdk)?) as Box<dyn Service>,
            _ => panic!("not found service"),
        };

        Ok(service)
    }

    fn list_service_name(&self) -> Vec<String> {
        vec!["asset".to_owned(), "mock".to_owned(), "metadata".to_owned()]
    }
}
