#![feature(async_await, await_macro, futures_api)]

use std::sync::Arc;

use futures01::future::{ok, Future as Future01};
use futures01::sync::mpsc::channel;
use logger;

use components_database::memory::MemoryDB;
use components_executor::evm::{EVMBlockDataProvider, EVMExecutor};
use components_executor::TrieDB;
use components_transaction_pool::HashTransactionPool;

use core_consensus::{
    Consensus, ConsensusStatus, Engine, FutConsensusResult, ProposalMessage, Status, Synchronizer,
    SynchronizerError, VoteMessage,
};
use core_context::{Context, P2P_SESSION_ID};
use core_crypto::{
    secp256k1::{PrivateKey, Secp256k1},
    Crypto, CryptoTransform,
};
use core_network::reactor::{outbound, CallbackMap, InboundReactor, JoinReactor, OutboundReactor};
use core_network::Config as NetworkConfig;
use core_network::Network;
use core_pubsub::PubSub;
use core_runtime::{FutRuntimeResult, TransactionPool};
use core_storage::{BlockStorage, Storage};
use core_types::{Address, Block, Transaction, UnverifiedTransaction};

#[derive(Debug, Clone)]
struct Config {
    // network
    bootstrap_addresses: Vec<String>,
    listening_address:   String,

    // transaction pool
    pool_size:         u64,
    until_block_limit: u64,
    quota_limit:       u64,
    height:            u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bootstrap_addresses: vec![],
            listening_address:   "/ip4/127.0.0.1/tcp/1337".to_owned(),

            pool_size:         100_000,
            until_block_limit: 100,
            quota_limit:       9_999_999_999,
            height:            100,
        }
    }
}

fn main() {
    logger::init(logger::Flag::Main);
    let config = Config::default();

    let ctx = Context::new();

    let untx = mock_transaction(
        100,
        config.height + config.until_block_limit,
        "test_test".to_owned(),
    );
    let tx_hash = untx.transaction.hash();

    // peer1
    let (peer1_tx_pool, _peer1_network) = start_peer(&config, false);

    // peer2
    let mut peer2_config = config.clone();
    peer2_config.bootstrap_addresses = vec!["/ip4/127.0.0.1/tcp/1337".to_owned()];
    peer2_config.listening_address = "/ip4/127.0.0.1/tcp/2337".to_owned();

    // FIXME: remove outbound reactor
    let (peer2_tx_pool, _peer2_network) = start_peer(&peer2_config, false);

    // test broadcast txs from peer1 to peer2
    peer1_tx_pool
        .insert(ctx.clone(), tx_hash.clone(), untx)
        .wait()
        .unwrap();

    std::thread::sleep(std::time::Duration::from_secs(3));

    let txs = peer2_tx_pool
        .get_batch(ctx.clone(), vec![tx_hash.clone()].as_slice())
        .wait()
        .unwrap();

    assert!(!txs.is_empty());
    println!("{:?}", txs);

    // =============== test "ensure" ====================

    // test "ensure" from peer2 to peer1
    let untx = mock_transaction(
        100,
        config.height + config.until_block_limit,
        "test_ensure".to_owned(),
    );
    let tx_hash = untx.transaction.hash();

    peer2_tx_pool
        .insert(ctx.clone(), tx_hash.clone(), untx)
        .wait()
        .unwrap();

    // here we assume both peers' session id is 1, since we only have 2 peers
    let peer1_ctx = ctx.with_value::<usize>(P2P_SESSION_ID, 1);
    peer1_tx_pool
        .ensure(peer1_ctx.clone(), vec![tx_hash.clone()].as_slice())
        .wait()
        .unwrap();

    let txs = peer1_tx_pool
        .get_batch(peer1_ctx.clone(), vec![tx_hash.clone()].as_slice())
        .wait()
        .unwrap();

    assert!(!txs.is_empty());
    println!("{:?}", txs);
}

fn start_peer(
    cfg: &Config,
    disable_outbound: bool,
) -> (
    Arc<HashTransactionPool<BlockStorage<MemoryDB>, Secp256k1, outbound::Sender>>,
    Network,
) {
    // new context
    let ctx = Context::new();

    // new crypto
    let secp = Arc::new(Secp256k1::new());

    // new db
    let block_db = Arc::new(MemoryDB::new());

    // new storage
    let storage = Arc::new(BlockStorage::new(Arc::clone(&block_db)));

    let mut block = Block::default();
    block.header.height = cfg.height;
    storage.insert_block(ctx.clone(), block).wait().unwrap();

    let (outbound_tx, outbound_rx) = channel(255);
    let outbound_tx = outbound::Sender::new(outbound_tx);

    // new tx pool
    let tx_pool = Arc::new(HashTransactionPool::new(
        Arc::clone(&storage),
        Arc::clone(&secp),
        outbound_tx,
        cfg.pool_size as usize,
        cfg.until_block_limit,
        cfg.quota_limit,
    ));

    // net network
    let callback_map = CallbackMap::default();
    let pubsub = PubSub::builder().build().start();
    let privkey = PrivateKey::from_bytes([0u8; 32].as_ref()).unwrap();
    let status = ConsensusStatus::default();
    let state_db = Arc::new(MemoryDB::new());
    let trie_db = Arc::new(TrieDB::new(Arc::clone(&state_db)));
    let block = Block::default();
    let executor = Arc::new(
        EVMExecutor::from_existing(
            trie_db,
            Arc::new(EVMBlockDataProvider::new(Arc::clone(&storage))),
            &block.header.state_root,
        )
        .unwrap(),
    );
    let engine = Arc::new(
        Engine::new(
            Arc::clone(&executor),
            Arc::clone(&tx_pool),
            Arc::clone(&storage),
            Arc::clone(&secp),
            privkey.clone(),
            status,
            pubsub.register(),
        )
        .unwrap(),
    );
    let inbound_reactor = InboundReactor::new(
        Arc::clone(&tx_pool),
        Arc::clone(&storage),
        engine,
        Arc::new(MockSynchronizer::default()),
        Arc::new(MockConsensus::default()),
        Arc::clone(&callback_map),
    );
    let outbound_reactor = OutboundReactor::new(callback_map);

    let mut network_config = NetworkConfig::default();
    network_config.p2p.listening_address = Some(cfg.listening_address.clone());
    network_config.p2p.bootstrap_addresses = cfg.bootstrap_addresses.clone();
    let network = {
        if disable_outbound {
            let network_reactor = inbound_reactor;
            Network::new(network_config, outbound_rx, network_reactor).unwrap()
        } else {
            let network_reactor = inbound_reactor.join(outbound_reactor);
            Network::new(network_config, outbound_rx, network_reactor).unwrap()
        }
    };

    (tx_pool, network)
}

fn mock_transaction(quota: u64, valid_until_block: u64, nonce: String) -> UnverifiedTransaction {
    let secp = Secp256k1::new();
    let (privkey, _pubkey) = secp.gen_keypair();
    let mut tx = Transaction::default();
    tx.to = Some(
        Address::from_bytes(
            hex::decode("ffffffffffffffffffffffffffffffffffffffff")
                .unwrap()
                .as_ref(),
        )
        .unwrap(),
    );
    tx.nonce = nonce;
    tx.quota = quota;
    tx.valid_until_block = valid_until_block;
    tx.data = vec![];
    tx.value = vec![];
    tx.chain_id = vec![];
    let tx_hash = tx.hash();

    let signature = secp.sign(&tx_hash, &privkey).unwrap();
    UnverifiedTransaction {
        transaction: tx,
        signature:   signature.as_bytes().to_vec(),
    }
}

#[derive(Default)]
pub struct MockConsensus {}

impl Consensus for MockConsensus {
    fn send_status(&self) -> FutConsensusResult<()> {
        Box::new(ok(()))
    }

    fn set_proposal(&self, _ctx: Context, _msg: ProposalMessage) -> FutConsensusResult<()> {
        Box::new(ok(()))
    }

    fn set_vote(&self, _ctx: Context, _msg: VoteMessage) -> FutConsensusResult<()> {
        Box::new(ok(()))
    }
}

#[derive(Default, Clone)]
pub struct MockSynchronizer {}

impl Synchronizer for MockSynchronizer {
    fn broadcast_status(&self, _status: Status) {}

    fn pull_blocks(
        &self,
        _ctx: Context,
        _heights: Vec<u64>,
    ) -> FutRuntimeResult<Vec<Block>, SynchronizerError> {
        unimplemented!()
    }
}
