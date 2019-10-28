#![feature(async_closure)]

mod config;

use std::convert::TryFrom;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;

use common_crypto::{PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey};
use core_api::adapter::DefaultAPIAdapter;
use core_api::config::GraphQLConfig;
use core_consensus::adapter::OverlordConsensusAdapter;
use core_consensus::consensus::OverlordConsensus;
use core_consensus::fixed_types::{ConsensusRpcResponse, FixedPill};
use core_consensus::message::{
    ProposalMessageHandler, QCMessageHandler, RichEpochIDMessageHandler, RpcHandler,
    VoteMessageHandler, END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_RICH_EPOCH_ID,
    END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE, RPC_RESP_SYNC_PULL, RPC_SYNC_PULL,
};
use core_executor::trie::RocksTrieDB;
use core_executor::TransactionExecutorFactory;
use core_mempool::{DefaultMemPoolAdapter, HashMemPool, NewTxsHandler, END_GOSSIP_NEW_TXS};
use core_network::{NetworkConfig, NetworkService};
use core_storage::{adapter::rocks::RocksAdapter, ImplStorage};

use protocol::traits::executor::ExecutorFactory;
use protocol::traits::{CurrentConsensusStatus, NodeInfo, Storage};
use protocol::types::{
    Address, Bloom, Epoch, EpochHeader, Genesis, Hash, MerkleRoot, Pill, Proof, UserAddress,
    Validator,
};
use protocol::ProtocolResult;

use crate::config::Config;

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    common_logger::init(common_logger::Flag::Main);

    let matches = clap::App::new("Muta")
        .version("v0.1.0")
        .author("Cryptape Technologies <contact@cryptape.com>")
        .arg(
            clap::Arg::from_usage("-c --config=[FILE] 'a required file for the configuration'")
                .default_value("./devtools/chain/config.toml"),
        )
        .arg(
            clap::Arg::from_usage("-g --genesis=[FILE] 'a required file for the genesis json'")
                .default_value("./devtools/chain/genesis.json"),
        )
        .get_matches();
    let args_config = matches.value_of("config").unwrap();
    let cfg: Config = common_config_parser::parse(args_config).unwrap();
    log::info!("Go with config: {:?}", cfg);

    // init genesis
    let genesis_path = matches.value_of("genesis").unwrap();
    log::info!("Genesis path: {}", genesis_path);
    handle_init(&cfg, genesis_path).await.unwrap();

    start(&cfg).await.unwrap();
}

async fn handle_init(cfg: &Config, genesis_path: impl AsRef<Path>) -> ProtocolResult<()> {
    let chain_id = Hash::from_hex(&cfg.chain_id).unwrap();

    // self private key
    let my_privkey =
        Secp256k1PrivateKey::try_from(hex::decode(cfg.privkey.clone()).unwrap().as_ref()).unwrap();
    let my_pubkey = my_privkey.pub_key();
    let my_address = UserAddress::from_pubkey_bytes(my_pubkey.to_bytes()).unwrap();

    // Read genesis.
    let mut r = File::open(genesis_path).unwrap();
    let genesis: Genesis = serde_json::from_reader(&mut r).unwrap();
    log::info!("Genesis data: {:?}", genesis);

    // Init Block db
    let path_block = cfg.data_path_for_block();
    log::info!("Data path for block: {:?}", path_block);
    let rocks_adapter = Arc::new(RocksAdapter::new(path_block).unwrap());
    let storage = Arc::new(ImplStorage::new(Arc::clone(&rocks_adapter)));

    match storage.get_latest_epoch().await {
        Ok(_) => {
            log::info!("The Genesis block has been initialized.");
            return Ok(());
        }
        Err(e) => {
            if !e.to_string().contains("GetNone") {
                return Err(e);
            }
        }
    };

    // Init trie db
    let path_state = cfg.data_path_for_state();
    let trie_db = Arc::new(RocksTrieDB::new(path_state, cfg.executor.light).unwrap());

    // Init genesis
    let genesis_state_root = {
        let mut executor = TransactionExecutorFactory::from_root(
            chain_id.clone(),
            MerkleRoot::from_empty(),
            Arc::clone(&trie_db),
            0,
            cfg.consensus.cycles_price,
            Address::User(my_address),
        )?;

        executor.create_genesis(&genesis)?
    };

    // Build genesis block.
    let genesis_epoch_header = EpochHeader {
        chain_id:          chain_id.clone(),
        epoch_id:          0,
        pre_hash:          Hash::from_empty(),
        timestamp:         genesis.timestamp,
        logs_bloom:        Bloom::default(),
        order_root:        Hash::from_empty(),
        confirm_root:      vec![],
        state_root:        genesis_state_root,
        receipt_root:      vec![Hash::from_empty()],
        cycles_used:       0,
        proposer:          UserAddress::from_hex("100000000000000000000000000000000000000000")
            .unwrap(),
        proof:             Proof {
            epoch_id:   0,
            round:      0,
            epoch_hash: Hash::from_empty(),
            signature:  Bytes::new(),
            bitmap:     Bytes::new(),
        },
        validator_version: 0,
        validators:        vec![],
    };
    let latest_proof = genesis_epoch_header.proof.clone();
    storage
        .insert_epoch(Epoch {
            header:            genesis_epoch_header,
            ordered_tx_hashes: vec![],
        })
        .await
        .unwrap();
    storage.update_latest_proof(latest_proof).await.unwrap();
    Ok(())
}

async fn start(cfg: &Config) -> ProtocolResult<()> {
    let chain_id = Hash::from_hex(&cfg.chain_id).unwrap();

    // self private key
    let my_privkey =
        Secp256k1PrivateKey::try_from(hex::decode(cfg.privkey.clone()).unwrap().as_ref()).unwrap();
    let my_pubkey = my_privkey.pub_key();
    let my_address = UserAddress::from_pubkey_bytes(my_pubkey.to_bytes()).unwrap();

    // Init Block db
    let path_block = cfg.data_path_for_block();
    log::info!("Data path for block: {:?}", path_block);
    let rocks_adapter = Arc::new(RocksAdapter::new(path_block).unwrap());
    let storage = Arc::new(ImplStorage::new(Arc::clone(&rocks_adapter)));

    // Init network
    let network_config = NetworkConfig::new();
    let network_privkey = cfg.privkey.clone();

    let mut bootstrap_pairs = vec![];
    if let Some(bootstrap) = &cfg.network.bootstraps {
        for bootstrap in bootstrap.iter() {
            bootstrap_pairs.push((bootstrap.pubkey.to_owned(), bootstrap.address));
        }
    }

    let network_config = network_config
        .bootstraps(bootstrap_pairs)
        .unwrap()
        .secio_keypair(network_privkey)
        .unwrap();
    let mut network_service = NetworkService::new(network_config);
    network_service
        .listen(cfg.network.listening_address)
        .unwrap();

    // Init mempool
    let current_epoch = storage.get_latest_epoch().await.unwrap();
    let mempool_adapter = DefaultMemPoolAdapter::<Secp256k1, _, _>::new(
        network_service.handle(),
        Arc::clone(&storage),
        cfg.mempool.timeout_gap,
        cfg.mempool.broadcast_txs_size,
        cfg.mempool.broadcast_txs_interval,
    );
    let mempool = Arc::new(HashMemPool::new(
        cfg.mempool.pool_size as usize,
        cfg.mempool.timeout_gap,
        mempool_adapter,
    ));

    // register broadcast new transaction
    network_service
        .register_endpoint_handler(
            END_GOSSIP_NEW_TXS,
            Box::new(NewTxsHandler::new(Arc::clone(&mempool))),
        )
        .unwrap();

    // Init trie db
    let path_state = cfg.data_path_for_state();
    let trie_db = Arc::new(RocksTrieDB::new(path_state, cfg.executor.light).unwrap());

    // Init Consensus
    let consensus_adapter = Arc::new(OverlordConsensusAdapter::<
        TransactionExecutorFactory,
        _,
        _,
        _,
        _,
        _,
    >::new(
        Arc::new(network_service.handle()),
        Arc::new(network_service.handle()),
        Arc::clone(&mempool),
        Arc::clone(&storage),
        Arc::clone(&trie_db),
    ));
    let node_info = NodeInfo {
        chain_id:     chain_id.clone(),
        self_address: my_address.clone(),
    };
    let current_header = &current_epoch.header;

    let prevhash = Hash::digest(Bytes::from(rlp::encode(&FixedPill {
        inner: Pill {
            epoch:          current_epoch.clone(),
            propose_hashes: vec![],
        },
    })));

    let current_consensus_status = CurrentConsensusStatus {
        cycles_price:       cfg.consensus.cycles_price,
        cycles_limit:       cfg.consensus.cycles_limit,
        epoch_id:           current_epoch.header.epoch_id + 1,
        prev_hash:          prevhash,
        logs_bloom:         current_header.logs_bloom,
        order_root:         Hash::from_empty(),
        confirm_root:       vec![Hash::from_empty()],
        state_root:         current_header.state_root.clone(),
        receipt_root:       vec![Hash::from_empty()],
        cycles_used:        current_header.cycles_used,
        proof:              current_header.proof.clone(),
        validators:         cfg
            .consensus
            .verifier_list
            .iter()
            .map(|v| Validator {
                address:        UserAddress::from_hex(v).unwrap(),
                propose_weight: 1,
                vote_weight:    1,
            })
            .collect(),
        consensus_interval: cfg.consensus.interval,
    };

    let overlord_consensus = Arc::new(OverlordConsensus::new(
        current_consensus_status,
        node_info,
        my_privkey,
        consensus_adapter,
    ));

    // register consensus
    network_service
        .register_endpoint_handler(
            END_GOSSIP_SIGNED_PROPOSAL,
            Box::new(ProposalMessageHandler::new(Arc::clone(&overlord_consensus))),
        )
        .unwrap();
    network_service
        .register_endpoint_handler(
            END_GOSSIP_AGGREGATED_VOTE,
            Box::new(QCMessageHandler::new(Arc::clone(&overlord_consensus))),
        )
        .unwrap();
    network_service
        .register_endpoint_handler(
            END_GOSSIP_SIGNED_VOTE,
            Box::new(VoteMessageHandler::new(Arc::clone(&overlord_consensus))),
        )
        .unwrap();
    network_service
        .register_endpoint_handler(
            END_GOSSIP_RICH_EPOCH_ID,
            Box::new(RichEpochIDMessageHandler::new(Arc::clone(
                &overlord_consensus,
            ))),
        )
        .unwrap();
    network_service
        .register_endpoint_handler(
            RPC_SYNC_PULL,
            Box::new(RpcHandler::new(
                Arc::new(network_service.handle()),
                Arc::clone(&storage),
            )),
        )
        .unwrap();
    network_service
        .register_rpc_response::<ConsensusRpcResponse>(RPC_RESP_SYNC_PULL)
        .unwrap();

    // Run network
    runtime::spawn(network_service);

    // Init graphql
    let api_adapter = DefaultAPIAdapter::<TransactionExecutorFactory, _, _, _>::new(
        Arc::clone(&mempool),
        Arc::clone(&storage),
        Arc::clone(&trie_db),
    );
    let mut graphql_config = GraphQLConfig::default();
    graphql_config.listening_address = cfg.graphql.listening_address;
    graphql_config.graphql_uri = cfg.graphql.graphql_uri.clone();
    graphql_config.graphiql_uri = cfg.graphql.graphiql_uri.clone();

    // Run GraphQL server
    runtime::spawn(core_api::start_graphql(graphql_config, api_adapter));

    // Run consensus
    overlord_consensus
        .run(cfg.consensus.interval, Some(cfg.consensus.duration.clone()))
        .await
        .unwrap();

    Ok(())
}
