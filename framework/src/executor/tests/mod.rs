use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use cita_trie::MemoryDB;

use asset::types::{Asset, GetBalanceResponse};
use protocol::traits::{Executor, ExecutorParams, Storage};
use protocol::types::{
    Address, Epoch, GenesisService, Hash, Proof, RawTransaction, Receipt, SignedTransaction,
    TransactionRequest,
};
use protocol::ProtocolResult;

use crate::executor::ServiceExecutor;

#[test]
fn test_create_genesis() {
    let yaml = include_str!("./genesis_services.yaml");
    let genesis_services: Vec<GenesisService> = serde_yaml::from_str(yaml).unwrap();
    let db = Arc::new(MemoryDB::new(false));

    let root = ServiceExecutor::create_genesis(
        genesis_services,
        Arc::clone(&db),
        Arc::new(MockStorage {}),
    )
    .unwrap();

    let executor =
        ServiceExecutor::with_root(root.clone(), Arc::clone(&db), Arc::new(MockStorage {}))
            .unwrap();
    let params = ExecutorParams {
        state_root:   root.clone(),
        epoch_id:     1,
        timestamp:    0,
        cycels_limit: std::u64::MAX,
    };
    let caller = Address::from_hex("f8389d774afdad8755ef8e629e5a154fddc6325a").unwrap();
    let request = TransactionRequest {
        service_name: "asset".to_owned(),
        method:       "get_balance".to_owned(),
        payload:
            r#"{"asset_id": "f56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c"}"#
                .to_owned(),
    };
    let res = executor.read(&params, &caller, 1, &request).unwrap();
    let resp: GetBalanceResponse = serde_json::from_str(&res.ret).unwrap();

    assert_eq!(resp.balance, 320000011);
}

#[test]
fn test_exec() {
    let yaml = include_str!("./genesis_services.yaml");
    let genesis_services: Vec<GenesisService> = serde_yaml::from_str(yaml).unwrap();
    let db = Arc::new(MemoryDB::new(false));

    let root = ServiceExecutor::create_genesis(
        genesis_services,
        Arc::clone(&db),
        Arc::new(MockStorage {}),
    )
    .unwrap();

    let mut executor =
        ServiceExecutor::with_root(root.clone(), Arc::clone(&db), Arc::new(MockStorage {}))
            .unwrap();

    let params = ExecutorParams {
        state_root:   root.clone(),
        epoch_id:     1,
        timestamp:    0,
        cycels_limit: std::u64::MAX,
    };

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
    let stx = SignedTransaction {
        raw,
        tx_hash: Hash::from_empty(),
        pubkey: Bytes::from(
            hex::decode("031288a6788678c25952eba8693b2f278f66e2187004b64ac09416d07f83f96d5b")
                .unwrap(),
        ),
        signature: Bytes::from("".as_bytes()),
    };
    let txs = vec![stx];
    let executor_resp = executor.exec(&params, &txs).unwrap();
    let receipt = &executor_resp.receipts[0];

    assert_eq!(receipt.response.is_error, false);
    let asset: Asset = serde_json::from_str(&receipt.response.ret).unwrap();
    assert_eq!(asset.name, "MutaToken2");
    assert_eq!(asset.symbol, "MT2");
    assert_eq!(asset.supply, 320000011);
}

struct MockStorage;

#[async_trait]
impl Storage for MockStorage {
    async fn insert_transactions(&self, _: Vec<SignedTransaction>) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn insert_epoch(&self, _: Epoch) -> ProtocolResult<()> {
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

    async fn get_latest_epoch(&self) -> ProtocolResult<Epoch> {
        unimplemented!()
    }

    async fn get_epoch_by_epoch_id(&self, _: u64) -> ProtocolResult<Epoch> {
        unimplemented!()
    }

    async fn get_epoch_by_hash(&self, _: Hash) -> ProtocolResult<Epoch> {
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
}
