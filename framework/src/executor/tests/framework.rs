use crate::executor::ServiceExecutor;

use async_trait::async_trait;
use binding_macro::{cycles, service, tx_hook_after, tx_hook_before};
use bytes::{Bytes, BytesMut};
use cita_trie::MemoryDB;
use protocol::traits::{
    CommonStorage, Context, Executor, ExecutorParams, ExecutorResp, SDKFactory, Service,
    ServiceMapping, ServiceResponse, ServiceSDK, Storage,
};
use protocol::types::{
    Address, Block, Genesis, Hash, Proof, RawTransaction, Receipt, ServiceContext,
    SignedTransaction, TransactionRequest,
};
use protocol::ProtocolResult;
use std::sync::Arc;

lazy_static::lazy_static! {
   pub static ref ADMIN_ACCOUNT: Address = "muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705".parse().unwrap();
}

macro_rules! exec_txs {
    ($exec_cycle_limit: expr, $tx_cycle_limit: expr $(, ($service: expr, $method: expr, $payload: expr))*) => {
        {
            let memdb = Arc::new(MemoryDB::new(false));
            let arcs = Arc::new(MockStorage {});

            let toml_str = include_str!("./framework_genesis_services.toml");
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
                cycles_limit: $exec_cycle_limit,
                proposer:     ADMIN_ACCOUNT.clone(),
            };

            let mut stxs = Vec::new();
            $(stxs.push(construct_stx(
                    $tx_cycle_limit,
                    $service.to_owned(),
                    $method.to_owned(),
                    serde_json::to_string(&$payload).unwrap()
                ));
            )*

            let resp : ExecutorResp  = executor.exec(Context::new(), &params, &stxs).unwrap();

            resp
        }
    };
}

pub fn construct_stx(
    tx_cycle_limit: u64,
    service_name: String,
    method: String,
    payload: String,
) -> SignedTransaction {
    let raw_tx = RawTransaction {
        chain_id:     Hash::from_empty(),
        nonce:        Hash::from_empty(),
        timeout:      0,
        cycles_price: 1,
        cycles_limit: tx_cycle_limit,
        request:      TransactionRequest {
            service_name,
            method,
            payload,
        },
        sender:       ADMIN_ACCOUNT.clone(),
    };

    SignedTransaction {
        raw:       raw_tx,
        tx_hash:   Hash::from_empty(),
        pubkey:    Bytes::from(
            hex::decode("031288a6788678c25952eba8693b2f278f66e2187004b64ac09416d07f83f96d5b")
                .unwrap(),
        ),
        signature: BytesMut::from("").freeze(),
    }
}

struct MockStorage;

#[async_trait]
impl CommonStorage for MockStorage {
    async fn insert_block(&self, _ctx: Context, _block: Block) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn get_block(&self, _ctx: Context, _height: u64) -> ProtocolResult<Option<Block>> {
        unimplemented!()
    }

    async fn set_block(&self, _ctx: Context, _block: Block) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn remove_block(&self, _ctx: Context, _height: u64) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn get_latest_block(&self, _ctx: Context) -> ProtocolResult<Block> {
        unimplemented!()
    }

    async fn set_latest_block(&self, _ctx: Context, _block: Block) -> ProtocolResult<()> {
        unimplemented!()
    }
}

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
}

pub struct MockServiceMapping;

impl ServiceMapping for MockServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK, Factory: SDKFactory<SDK>>(
        &self,
        name: &str,
        factory: &Factory,
    ) -> ProtocolResult<Box<dyn Service>> {
        let sdk = factory.get_sdk(name)?;

        let service = match name {
            "TestService" => Box::new(TestService::new(sdk)) as Box<dyn Service>,
            _ => panic!("not found service"),
        };

        Ok(service)
    }

    fn list_service_name(&self) -> Vec<String> {
        vec!["TestService".to_owned()]
    }
}

pub struct TestService<SDK> {
    _sdk: SDK,
}

#[service]
impl<SDK: ServiceSDK> TestService<SDK> {
    pub fn new(sdk: SDK) -> Self {
        Self { _sdk: sdk }
    }

    #[cycles(10_000)]
    #[read]
    fn test_read(&self, _ctx: ServiceContext) -> ServiceResponse<String> {
        ServiceResponse::from_succeed("".to_owned())
    }

    #[cycles(300_00)]
    #[write]
    fn test_write(&mut self, ctx: ServiceContext) -> ServiceResponse<String> {
        ctx.emit_event(
            "test_service".to_owned(),
            "write".to_owned(),
            "write".to_owned(),
        );
        ServiceResponse::from_succeed("".to_owned())
    }

    #[tx_hook_before]
    fn test_tx_hook_before(&mut self, ctx: ServiceContext) -> ServiceResponse<()> {
        // we emit an event
        ctx.emit_event(
            "test_service".to_owned(),
            "before".to_owned(),
            "before".to_owned(),
        );
        if ctx.get_payload().contains("before") {
            return ServiceResponse::from_error(2, "before_error".to_owned());
        }
        ServiceResponse::from_succeed(())
    }

    #[tx_hook_after]
    fn test_tx_hook_after(&mut self, ctx: ServiceContext) -> ServiceResponse<()> {
        if ctx.get_payload().contains("after") {
            return ServiceResponse::from_error(2, "after_error".to_owned());
        }
        ctx.emit_event(
            "test_service".to_owned(),
            "after".to_owned(),
            "after".to_owned(),
        );
        ServiceResponse::from_succeed(())
    }
}

#[test]
fn test_tx_hook_ok_ok() {
    let resp: ExecutorResp =
        exec_txs!(50000, 50000, ("TestService", "test_write", "a test string"));
    assert_eq!(3, resp.receipts.get(0).unwrap().events.len());

    let resp: ExecutorResp = exec_txs!(50000, 50000, ("TestService", "test_write", "before"));
    assert_eq!(2, resp.receipts.get(0).unwrap().events.len());
    assert!(resp
        .receipts
        .get(0)
        .unwrap()
        .events
        .iter()
        .any(|e| { e.name.as_str() == "after" }));
    assert!(resp
        .receipts
        .get(0)
        .unwrap()
        .events
        .iter()
        .any(|e| { e.name.as_str() == "before" }));

    let resp: ExecutorResp = exec_txs!(50000, 50000, ("TestService", "test_write", "after"));
    assert_eq!(1, resp.receipts.get(0).unwrap().events.len());
    assert!(resp
        .receipts
        .get(0)
        .unwrap()
        .events
        .iter()
        .any(|e| { e.name.as_str() == "before" }));

    let resp: ExecutorResp = exec_txs!(50000, 50000, ("TestService", "test_write", "before_after"));
    assert_eq!(1, resp.receipts.get(0).unwrap().events.len());
    assert!(resp
        .receipts
        .get(0)
        .unwrap()
        .events
        .iter()
        .any(|e| { e.name.as_str() == "before" }));
}
