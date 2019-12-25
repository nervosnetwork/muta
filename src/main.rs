#![feature(async_closure)]

mod config;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::File;
use std::path::Path;
use std::str::from_utf8;
use std::sync::Arc;

use common_crypto::{
    BlsCommonReference, BlsPrivateKey, BlsPublicKey, PublicKey, Secp256k1, Secp256k1PrivateKey,
    ToPublicKey,
};
use core_api::adapter::DefaultAPIAdapter;
use core_api::config::GraphQLConfig;
use core_consensus::fixed_types::{FixedEpoch, FixedSignedTxs};
use core_consensus::message::{
    ProposalMessageHandler, PullEpochRpcHandler, PullTxsRpcHandler, QCMessageHandler,
    RichEpochIDMessageHandler, VoteMessageHandler, END_GOSSIP_AGGREGATED_VOTE,
    END_GOSSIP_RICH_EPOCH_ID, END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE,
    RPC_RESP_SYNC_PULL_EPOCH, RPC_RESP_SYNC_PULL_TXS, RPC_SYNC_PULL_EPOCH, RPC_SYNC_PULL_TXS,
};
use core_consensus::status::{CurrentConsensusStatus, StatusPivot};
use core_consensus::trace::init_tracer;
use core_consensus::{OverlordConsensus, OverlordConsensusAdapter};
use core_executor::trie::RocksTrieDB;
use core_executor::TransactionExecutorFactory;
use core_mempool::{
    DefaultMemPoolAdapter, HashMemPool, MsgPushTxs, NewTxsHandler, PullTxsHandler,
    END_GOSSIP_NEW_TXS, RPC_PULL_TXS, RPC_RESP_PULL_TXS,
};
use core_network::{NetworkConfig, NetworkService};
use core_storage::{adapter::rocks::RocksAdapter, ImplStorage};
use futures::executor::block_on;
use parking_lot::RwLock;

use protocol::traits::executor::ExecutorFactory;
use protocol::traits::{NodeInfo, Storage};
use protocol::types::{
    Address, Bloom, Epoch, EpochHeader, Genesis, Hash, MerkleRoot, Proof, UserAddress, Validator,
};
use protocol::{fixed_codec::ProtocolFixedCodec, Bytes, ProtocolResult};

use crate::config::Config;

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    let matches = clap::App::new("Muta")
        .version("v0.1.0")
        .author("Muta Dev <muta@nervos.org>")
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
    common_logger::init(
        cfg.logger.filter.clone(),
        cfg.logger.log_to_console,
        cfg.logger.console_show_file_and_line,
        cfg.logger.log_to_file,
        cfg.logger.metrics,
        cfg.logger.log_path.clone(),
    );
    log::info!("Go with config: {:?}", cfg);

    // init genesis
    let genesis_path = matches.value_of("genesis").unwrap();
    log::info!("Genesis path: {}", genesis_path);
    handle_init(&cfg, genesis_path).await.unwrap();

    start(cfg).await.unwrap();
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
        logs_bloom:        vec![Bloom::default()],
        order_root:        Hash::from_empty(),
        confirm_root:      vec![],
        state_root:        genesis_state_root,
        receipt_root:      vec![Hash::from_empty()],
        cycles_used:       vec![0],
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

async fn start(cfg: Config) -> ProtocolResult<()> {
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

    // register pull txs from other node
    network_service
        .register_endpoint_handler(
            RPC_PULL_TXS,
            Box::new(PullTxsHandler::new(
                Arc::new(network_service.handle()),
                Arc::clone(&mempool),
            )),
        )
        .unwrap();
    network_service
        .register_rpc_response::<MsgPushTxs>(RPC_RESP_PULL_TXS)
        .unwrap();

    // Init trie db
    let path_state = cfg.data_path_for_state();
    let trie_db = Arc::new(RocksTrieDB::new(path_state, cfg.executor.light).unwrap());

    // Init Consensus
    let node_info = NodeInfo {
        chain_id:     chain_id.clone(),
        self_address: my_address.clone(),
    };
    let current_header = &current_epoch.header;
    let prevhash = Hash::digest(current_epoch.encode_fixed()?);

    let current_consensus_status = Arc::new(RwLock::new(CurrentConsensusStatus {
        cycles_price:       cfg.consensus.cycles_price,
        cycles_limit:       cfg.consensus.cycles_limit,
        epoch_id:           current_epoch.header.epoch_id + 1,
        exec_epoch_id:      current_epoch.header.epoch_id,
        prev_hash:          prevhash,
        logs_bloom:         current_header.logs_bloom.clone(),
        confirm_root:       vec![],
        state_root:         vec![current_header.state_root.clone()],
        receipt_root:       vec![],
        cycles_used:        current_header.cycles_used.clone(),
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
    }));

    assert!(cfg.consensus.verifier_list.len() == cfg.consensus.public_keys.len());
    let mut bls_pub_keys = HashMap::new();
    for (addr, bls_pub_key) in cfg
        .consensus
        .verifier_list
        .iter()
        .zip(cfg.consensus.public_keys.iter())
    {
        let address = UserAddress::from_hex(addr).unwrap().as_bytes();
        let pub_key = BlsPublicKey::try_from(hex::decode(bls_pub_key).unwrap().as_ref()).unwrap();
        bls_pub_keys.insert(address, pub_key);
    }

    let bls_priv_key = BlsPrivateKey::try_from(
        hex::decode(cfg.consensus.private_key.clone())
            .unwrap()
            .as_ref(),
    )
    .unwrap();

    let common_ref: BlsCommonReference = from_utf8(
        hex::decode(cfg.consensus.common_ref.as_str())
            .unwrap()
            .as_ref(),
    )
    .unwrap()
    .into();

    init_tracer(my_address.as_hex()).unwrap();
    let (status_pivot, agent) = StatusPivot::new(Arc::clone(&current_consensus_status));

    let mut consensus_adapter =
        OverlordConsensusAdapter::<TransactionExecutorFactory, _, _, _, _, _>::new(
            Arc::new(network_service.handle()),
            Arc::new(network_service.handle()),
            Arc::clone(&mempool),
            Arc::clone(&storage),
            Arc::clone(&trie_db),
            agent,
            current_header.state_root.clone(),
        );

    let exec_demon = consensus_adapter.take_exec_demon();
    let consensus_adapter = Arc::new(consensus_adapter);

    let (tmp, synchronization) = OverlordConsensus::new(
        current_consensus_status,
        node_info,
        bls_pub_keys,
        bls_priv_key,
        common_ref,
        consensus_adapter,
    );

    let overlord_consensus = Arc::new(tmp);

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
            RPC_SYNC_PULL_EPOCH,
            Box::new(PullEpochRpcHandler::new(
                Arc::new(network_service.handle()),
                Arc::clone(&storage),
            )),
        )
        .unwrap();
    network_service
        .register_endpoint_handler(
            RPC_SYNC_PULL_TXS,
            Box::new(PullTxsRpcHandler::new(
                Arc::new(network_service.handle()),
                Arc::clone(&storage),
            )),
        )
        .unwrap();
    network_service
        .register_rpc_response::<FixedEpoch>(RPC_RESP_SYNC_PULL_EPOCH)
        .unwrap();
    network_service
        .register_rpc_response::<FixedSignedTxs>(RPC_RESP_SYNC_PULL_TXS)
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

    // Run sychronization process
    runtime::spawn(synchronization.run());

    // Run status cache pivot
    runtime::spawn(status_pivot.run());

    // Run consensus
    runtime::spawn(async move {
        if let Err(e) = overlord_consensus
            .run(cfg.consensus.interval, Some(cfg.consensus.duration.clone()))
            .await
        {
            log::error!("muta-consensus: {:?} error", e);
        }
    });

    // Run execute demon
    block_on(exec_demon.run());
    Ok(())
}
