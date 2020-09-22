#![allow(clippy::needless_collect)]
#![feature(test)]
extern crate test;

use std::str::FromStr;
use std::sync::Arc;

use common_crypto::{Crypto, Secp256k1, Signature};
use core_mempool::DefaultMemPoolAdapter;
use core_network::{NetworkConfig, NetworkService, NetworkServiceHandle};
use core_storage::{adapter::rocks::RocksAdapter, ImplStorage};
use framework::binding::state::RocksTrieDB;
use framework::executor::{ServiceExecutor, ServiceExecutorFactory};
use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    CommonStorage, Context, Executor, ExecutorParams, SDKFactory, Service, ServiceMapping,
    ServiceSDK, Storage,
};
use protocol::types::{
    Address, Block, BlockHeader, Bytes, Genesis, Hash, Hex, MerkleRoot, Proof, RawTransaction,
    SignedTransaction, TransactionRequest,
};
use protocol::ProtocolResult;
use test::Bencher;

use asset::AssetService;
use governance::GovernanceService;
use multi_signature::MultiSignatureService;

const TRIE_PATH: &str = "./free-space/state";
const STORAGE_PATH: &str = "./free-space/block";

lazy_static::lazy_static! {
    pub static ref FEE_ACCOUNT: Address = Address::from_str("muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705").unwrap();
    pub static ref FEE_INLET_ACCOUNT: Address = Address::from_str("muta15a8a9ksxe3hhjpw3l7wz7ry778qg8h9wz8y35p").unwrap();
    pub static ref PROPOSER_ACCOUNT: Address = Address::from_str("muta1h99h6f54vytatam3ckftrmvcdpn4jlmnwm6hl0").unwrap();
    pub static ref NATIVE_ASSET_ID: Hash = Hash::from_hex("0xf56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c").unwrap();
    pub static ref PRIV_KEY: Bytes = Hex::from_string("0x5ec982173d54d830b6789cbbbe43eaa2853a5ff752d1ebc1b266cf9790314f8a".to_string()).unwrap().decode();
    pub static ref PUB_KEY: Bytes = Hex::from_string(
        "0x02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60".to_string(),
    )
    .unwrap()
    .decode();
}

macro_rules! exec {
    ($adapter: expr, $payloads: expr) => {{
        let stxs = $payloads.into_iter().map(construct_stx).collect::<Vec<_>>();

        let mut executor = $adapter.create_executor();
        let params = $adapter.create_params();

        executor.exec(Context::new(), &params, &stxs).unwrap();
        $adapter.next_height();
    }};
}

macro_rules! perf_exec {
    ($adapter: expr, $payloads: expr, $bencher: expr) => {{
        let stxs = $payloads.into_iter().map(construct_stx).collect::<Vec<_>>();

        let mut executor = $adapter.create_executor();
        let params = $adapter.create_params();

        $bencher.iter(|| {
            let txs = stxs.clone();
            executor.exec(Context::new(), &params, &txs).unwrap();
        });
    }};
}

mod bench_executor;
mod bench_mempool;
// This is a test service that provides transaction hooks.
mod governance;

pub struct BenchmarkAdapter {
    trie_db:    Arc<RocksTrieDB>,
    storage:    Arc<ImplStorage<RocksAdapter>>,
    height:     u64,
    timestamp:  u64,
    state_root: MerkleRoot,
}

impl Default for BenchmarkAdapter {
    fn default() -> Self {
        BenchmarkAdapter::new()
    }
}

impl BenchmarkAdapter {
    pub fn new() -> Self {
        let mut rt = tokio::runtime::Builder::new()
            .core_threads(4)
            .build()
            .unwrap();
        let rocks_adapter = Arc::new(RocksAdapter::new(STORAGE_PATH, 1024).unwrap());
        let toml_str = include_str!("./benchmark_genesis.toml");
        let genesis: Genesis = toml::from_str(toml_str).unwrap();

        let mut ret = BenchmarkAdapter {
            trie_db:    Arc::new(RocksTrieDB::new(TRIE_PATH, false, 1024, 2000).unwrap()),
            storage:    Arc::new(ImplStorage::new(Arc::clone(&rocks_adapter))),
            height:     1,
            timestamp:  1,
            state_root: Hash::default(),
        };

        let root = ServiceExecutor::create_genesis(
            genesis.services,
            Arc::clone(&ret.trie_db),
            Arc::clone(&ret.storage),
            Arc::new(MockServiceMapping {}),
        )
        .unwrap();

        let genesis_block = BenchmarkAdapter::create_genesis_block(root.clone());

        rt.block_on(async {
            ret.storage
                .update_latest_proof(Context::new(), genesis_block.header.proof.clone())
                .await
                .expect("save proof");
            ret.storage
                .insert_block(Context::new(), genesis_block)
                .await
                .expect("save genesis");
        });

        ret.state_root = root;
        ret
    }

    pub fn create_executor(
        &mut self,
    ) -> ServiceExecutor<ImplStorage<RocksAdapter>, RocksTrieDB, MockServiceMapping> {
        ServiceExecutor::with_root(
            self.state_root.clone(),
            Arc::clone(&self.trie_db),
            Arc::clone(&self.storage),
            Arc::new(MockServiceMapping {}),
        )
        .unwrap()
    }

    pub fn create_params(&mut self) -> ExecutorParams {
        ExecutorParams {
            state_root:   self.state_root.clone(),
            height:       self.height,
            timestamp:    self.timestamp,
            cycles_limit: u64::max_value(),
            proposer:     PROPOSER_ACCOUNT.clone(),
        }
    }

    pub fn create_mempool_adapter(
        &mut self,
    ) -> DefaultMemPoolAdapter<
        ServiceExecutorFactory,
        Secp256k1,
        NetworkServiceHandle,
        ImplStorage<RocksAdapter>,
        RocksTrieDB,
        MockServiceMapping,
    > {
        DefaultMemPoolAdapter::new(
            NetworkService::new(NetworkConfig::new()).handle(),
            Arc::clone(&self.storage),
            Arc::clone(&self.trie_db),
            Arc::new(MockServiceMapping {}),
            3000,
            100,
        )
    }

    pub fn next_height(&mut self) {
        self.height += 1;
        self.timestamp += 2;
    }

    fn create_genesis_block(state_root: MerkleRoot) -> Block {
        let genesis_block_header = BlockHeader {
            chain_id: Hash::default(),
            height: 0,
            exec_height: 0,
            prev_hash: Hash::from_empty(),
            timestamp: 0,
            order_root: Hash::from_empty(),
            order_signed_transactions_hash: Hash::from_empty(),
            confirm_root: vec![],
            state_root,
            receipt_root: vec![],
            cycles_used: vec![],
            proposer: PROPOSER_ACCOUNT.clone(),
            proof: Proof {
                height:     0,
                round:      0,
                block_hash: Hash::from_empty(),
                signature:  Bytes::new(),
                bitmap:     Bytes::new(),
            },
            validator_version: 0,
            validators: vec![],
        };

        Block {
            header:            genesis_block_header,
            ordered_tx_hashes: vec![],
        }
    }
}

pub fn construct_stx(req: TransactionRequest) -> SignedTransaction {
    let raw_tx = RawTransaction {
        chain_id:     Hash::default(),
        nonce:        Hash::from_empty(),
        timeout:      300,
        cycles_price: 1,
        cycles_limit: u64::max_value(),
        request:      req,
        sender:       FEE_ACCOUNT.clone(),
    };

    let hash = Hash::digest(raw_tx.encode_fixed().unwrap());
    let sig = Secp256k1::sign_message(&hash.as_bytes(), &PRIV_KEY).unwrap();

    SignedTransaction {
        raw:       raw_tx,
        tx_hash:   hash,
        pubkey:    Bytes::from(rlp::encode_list::<Vec<u8>, _>(&[PUB_KEY.clone().to_vec()])),
        signature: Bytes::from(rlp::encode_list::<Vec<u8>, _>(&[sig.to_bytes().to_vec()])),
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
        let multi_sig_sdk = factory.get_sdk("multi_signature")?;

        let service = match name {
            "asset" => Box::new(AssetService::new(asset_sdk)) as Box<dyn Service>,

            "governance" => Box::new(GovernanceService::new(
                governance_sdk,
                AssetService::new(asset_sdk),
            )) as Box<dyn Service>,

            "multi_signature" => {
                Box::new(MultiSignatureService::new(multi_sig_sdk)) as Box<dyn Service>
            }

            _ => panic!("not found service"),
        };

        Ok(service)
    }

    fn list_service_name(&self) -> Vec<String> {
        vec![
            "asset".to_owned(),
            "governance".to_owned(),
            "multi_signature".to_owned(),
        ]
    }
}
