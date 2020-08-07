#![feature(test)]

extern crate test;

use std::str::FromStr;
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use core_storage::{adapter::rocks::RocksAdapter, ImplStorage};
use framework::binding::state::RocksTrieDB;
use framework::executor::ServiceExecutor;
use protocol::traits::{
    Context, Executor, ExecutorParams, SDKFactory, Service, ServiceMapping, ServiceSDK,
};
use protocol::types::{
    Address, Genesis, Hash, RawTransaction, SignedTransaction, TransactionRequest,
};
use protocol::ProtocolResult;
use test::Bencher;

use asset::AssetService;
use governance::GovernanceService;

lazy_static::lazy_static! {
   pub static ref ADMIN_ACCOUNT: Address = Address::from_str("muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705").unwrap();
   pub static ref FEE_ACCOUNT: Address = Address::from_str("muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705").unwrap();
   pub static ref FEE_INLET_ACCOUNT: Address = Address::from_str("muta15a8a9ksxe3hhjpw3l7wz7ry778qg8h9wz8y35p").unwrap();
   pub static ref PROPOSER_ACCOUNT: Address = Address::from_str("muta1h99h6f54vytatam3ckftrmvcdpn4jlmnwm6hl0").unwrap();
   pub static ref NATIVE_ASSET_ID: Hash = Hash::from_hex("0xf56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c").unwrap();
}

macro_rules! benchmark {
    ($payload: expr, $num: expr, $bencher: expr) => {{
        let memdb = Arc::new(RocksTrieDB::new("./free-space/state", false, 1024, 2000).unwrap());

        let rocks_adapter = Arc::new(RocksAdapter::new("./free-space/block", 1024).unwrap());

        let storage = Arc::new(ImplStorage::new(Arc::clone(&rocks_adapter)));
        let toml_str = include_str!("./benchmark_genesis.toml");
        let genesis: Genesis = toml::from_str(toml_str).unwrap();

        let root = ServiceExecutor::create_genesis(
            genesis.services,
            Arc::clone(&memdb),
            Arc::clone(&storage),
            Arc::new(MockServiceMapping {}),
        )
        .unwrap();

        let stxs = (0..$num)
            .map(|_| construct_stx($payload.clone()))
            .collect::<Vec<_>>();

        let params = ExecutorParams {
            state_root:   root.clone(),
            height:       1,
            timestamp:    0,
            cycles_limit: u64::max_value(),
            proposer:     PROPOSER_ACCOUNT.clone(),
        };

        let mut executor = ServiceExecutor::with_root(
            root,
            Arc::clone(&memdb),
            Arc::clone(&storage),
            Arc::new(MockServiceMapping {}),
        )
        .unwrap();

        $bencher.iter(|| {
            let txs = stxs.clone();
            executor.exec(Context::new(), &params, &txs).unwrap();
        });
    }};
}

macro_rules! perf_exec {
    ($assets_num: expr, $payload: expr, $num: expr, $bencher: expr) => {{
        let memdb = Arc::new(RocksTrieDB::new("./free-space/state", false, 1024, 2000).unwrap());

        let rocks_adapter = Arc::new(RocksAdapter::new("./free-space/block", 1024).unwrap());

        let storage = Arc::new(ImplStorage::new(Arc::clone(&rocks_adapter)));
        let toml_str = include_str!("./benchmark_genesis.toml");
        let genesis: Genesis = toml::from_str(toml_str).unwrap();

        let root = ServiceExecutor::create_genesis(
            genesis.services,
            Arc::clone(&memdb),
            Arc::clone(&storage),
            Arc::new(MockServiceMapping {}),
        )
        .unwrap();

        let create_asset_txs = (0..$assets_num)
            .map(|n| {
                let payload = asset::types::CreateAssetPayload {
                    name:   "muta_".to_string() + n.to_string().as_str(),
                    symbol: "muta_".to_string() + n.to_string().as_str(),
                    supply: 100_000,
                };

                construct_stx(TransactionRequest {
                    service_name: "asset".to_string(),
                    method:       "create_asset".to_string(),
                    payload:      serde_json::to_string(&payload).unwrap(),
                })
            })
            .collect::<Vec<_>>();

        let stxs = (0..$num)
            .map(|_| construct_stx($payload.clone()))
            .collect::<Vec<_>>();

        let mut params = ExecutorParams {
            state_root:   root.clone(),
            height:       1,
            timestamp:    0,
            cycles_limit: u64::max_value(),
            proposer:     PROPOSER_ACCOUNT.clone(),
        };

        let mut executor = ServiceExecutor::with_root(
            root,
            Arc::clone(&memdb),
            Arc::clone(&storage),
            Arc::new(MockServiceMapping {}),
        )
        .unwrap();

        let resp = executor
            .exec(Context::new(), &params, &create_asset_txs)
            .unwrap();
        params.state_root = resp.state_root;
        params.height += 1;
        params.timestamp += 2;

        $bencher.iter(|| {
            let txs = stxs.clone();
            executor.exec(Context::new(), &params, &txs).unwrap();
        });
    }};
}

mod bench;
// This is a test service that provides transaction hooks.
mod governance;

pub fn construct_stx(req: TransactionRequest) -> SignedTransaction {
    let raw_tx = RawTransaction {
        chain_id:     Hash::from_empty(),
        nonce:        Hash::from_empty(),
        timeout:      0,
        cycles_price: 1,
        cycles_limit: u64::max_value(),
        request:      req,
        sender:       FEE_ACCOUNT.clone(),
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

pub struct MockServiceMapping;

impl ServiceMapping for MockServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK, Factory: SDKFactory<SDK>>(
        &self,
        name: &str,
        factory: &Factory,
    ) -> ProtocolResult<Box<dyn Service>> {
        let asset_sdk = factory.get_sdk("asset")?;
        let governance_sdk = factory.get_sdk("governance")?;

        let service = match name {
            "asset" => Box::new(AssetService::new(asset_sdk)) as Box<dyn Service>,
            "governance" => Box::new(GovernanceService::new(
                governance_sdk,
                AssetService::new(asset_sdk),
            )) as Box<dyn Service>,
            _ => panic!("not found service"),
        };

        Ok(service)
    }

    fn list_service_name(&self) -> Vec<String> {
        vec!["asset".to_owned(), "governance".to_owned()]
    }
}
