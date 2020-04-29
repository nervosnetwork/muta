use std::convert::TryFrom;
use std::sync::Arc;

use bytes::Bytes;
#[cfg(unix)]
use tokio::signal::unix::{self as os_impl};

use common_crypto::{PublicKey, Secp256k1, Secp256k1PrivateKey, ToPublicKey};
use core_api::adapter::DefaultAPIAdapter;
use core_api::config::GraphQLConfig;
use core_consensus::message::{
    PreCommitQCHandler, PreVoteQCHandler, SignedChokeHandler, SignedHeightHandler,
    SignedPreCommitHandler, SignedPreVoteHandler, SignedProposalHandler, SyncRequestHandler,
    SyncResponseHandler, END_GOSSIP_PRE_COMMIT_QC, END_GOSSIP_PRE_VOTE_QC, END_GOSSIP_SIGNED_CHOKE,
    END_GOSSIP_SIGNED_HEIGHT, END_GOSSIP_SIGNED_PRE_COMMIT, END_GOSSIP_SIGNED_PRE_VOTE,
    END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SYNC_REQUEST, END_GOSSIP_SYNC_RESPONSE,
};
use core_consensus::OverlordAdapter;
use core_mempool::{
    DefaultMemPoolAdapter, HashMemPool, MsgPushTxs, NewTxsHandler, PullTxsHandler,
    END_GOSSIP_NEW_TXS, RPC_PULL_TXS, RPC_RESP_PULL_TXS, RPC_RESP_PULL_TXS_SYNC,
};
use core_network::{NetworkConfig, NetworkService};
use core_storage::{adapter::rocks::RocksAdapter, ImplStorage};
use framework::binding::state::RocksTrieDB;
use framework::executor::{ServiceExecutor, ServiceExecutorFactory};
use overlord::crypto::gen_keypair;
use overlord::OverlordServer;
use protocol::traits::{APIAdapter, Context, MemPool, ServiceMapping, Storage};
use protocol::types::{Address, Block, BlockHeader, Genesis, Hash, Metadata, Proof, Validator};
use protocol::ProtocolResult;

use crate::config::Config;
use crate::MainError;

pub async fn create_genesis<Mapping: 'static + ServiceMapping>(
    config: &Config,
    genesis: &Genesis,
    servive_mapping: Arc<Mapping>,
) -> ProtocolResult<Block> {
    let metadata: Metadata =
        serde_json::from_str(genesis.get_payload("metadata")).expect("Decode metadata failed!");

    let validators: Vec<Validator> = metadata
        .verifier_list
        .iter()
        .map(|v| Validator {
            address:        v.address.clone(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect();

    // Read genesis.
    log::info!("Genesis data: {:?}", genesis);

    // Init Block db
    let path_block = config.data_path_for_block();
    let rocks_adapter = Arc::new(RocksAdapter::new(
        path_block,
        config.rocksdb.max_open_files,
    )?);
    let storage = Arc::new(ImplStorage::new(Arc::clone(&rocks_adapter)));

    match storage.get_latest_block().await {
        Ok(genesis_block) => {
            log::info!("The Genesis block has been initialized.");
            return Ok(genesis_block);
        }
        Err(e) => {
            if !e.to_string().contains("GetNone") {
                return Err(e);
            }
        }
    };

    // Init trie db
    let path_state = config.data_path_for_state();
    let trie_db = Arc::new(RocksTrieDB::new(
        path_state,
        config.executor.light,
        config.rocksdb.max_open_files,
    )?);

    // Init genesis
    let genesis_state_root = ServiceExecutor::create_genesis(
        genesis.services.clone(),
        Arc::clone(&trie_db),
        Arc::clone(&storage),
        servive_mapping,
    )?;

    // Build genesis block.
    let genesis_block_header = BlockHeader {
        chain_id: metadata.chain_id.clone(),
        height: 0,
        exec_height: 0,
        pre_hash: Hash::from_empty(),
        timestamp: genesis.timestamp,
        logs_bloom: vec![],
        order_root: Hash::from_empty(),
        confirm_root: vec![],
        state_root: genesis_state_root,
        receipt_root: vec![],
        cycles_used: vec![],
        proposer: Address::from_hex("0x0000000000000000000000000000000000000000")?,
        proof: Proof {
            height:     0,
            round:      0,
            block_hash: Hash::from_empty(),
            signature:  Bytes::new(),
            bitmap:     Bytes::new(),
        },
        validator_version: 0,
        validators,
    };
    let latest_proof = genesis_block_header.proof.clone();
    let genesis_block = Block {
        header:            genesis_block_header,
        ordered_tx_hashes: vec![],
    };
    storage.insert_block(genesis_block.clone()).await?;
    storage.update_latest_proof(latest_proof).await?;

    log::info!("The genesis block is created {:?}", genesis_block);
    Ok(genesis_block)
}

pub async fn start<Mapping: 'static + ServiceMapping>(
    config: Config,
    service_mapping: Arc<Mapping>,
) -> ProtocolResult<()> {
    log::info!("node starts");
    // Init Block db
    let path_block = config.data_path_for_block();
    log::info!("Data path for block: {:?}", path_block);

    let rocks_adapter = Arc::new(RocksAdapter::new(
        path_block.clone(),
        config.rocksdb.max_open_files,
    )?);
    let storage = Arc::new(ImplStorage::new(Arc::clone(&rocks_adapter)));

    // Init network
    let network_config = NetworkConfig::new()
        .max_connections(config.network.max_connected_peers.clone())
        .whitelist_peers_only(config.network.whitelist_peers_only.clone())
        .peer_trust_metric(
            config.network.trust_interval_duration,
            config.network.trust_max_history_duration,
        )?
        .peer_soft_ban(config.network.soft_ban_duration)
        .peer_fatal_ban(config.network.fatal_ban_duration)
        .rpc_timeout(config.network.rpc_timeout.clone())
        .selfcheck_interval(config.network.selfcheck_interval.clone())
        .max_wait_streams(config.network.max_wait_streams)
        .max_frame_length(config.network.max_frame_length.clone())
        .send_buffer_size(config.network.send_buffer_size.clone())
        .write_timeout(config.network.write_timeout)
        .recv_buffer_size(config.network.recv_buffer_size.clone());

    let network_pri_key = config.privkey.as_string_trim0x();

    let mut bootstrap_pairs = vec![];
    if let Some(bootstrap) = &config.network.bootstraps {
        for bootstrap in bootstrap.iter() {
            bootstrap_pairs.push((
                bootstrap.pubkey.as_string_trim0x(),
                bootstrap.address.to_owned(),
            ));
        }
    }

    let whitelist = config.network.whitelist.clone().unwrap_or_default();

    let network_config = network_config
        .bootstraps(bootstrap_pairs)?
        .whitelist(whitelist)?
        .secio_keypair(network_pri_key)?;
    let mut network_service = NetworkService::new(network_config);
    network_service
        .listen(config.network.listening_address)
        .await?;

    // Init mempool
    let current_block = storage.get_latest_block().await?;
    let mempool_adapter = DefaultMemPoolAdapter::<Secp256k1, _, _>::new(
        network_service.handle(),
        Arc::clone(&storage),
        config.mempool.broadcast_txs_size,
        config.mempool.broadcast_txs_interval,
    );
    let mempool = Arc::new(HashMemPool::new(
        config.mempool.pool_size as usize,
        mempool_adapter,
    ));

    // Init trie db
    let path_state = config.data_path_for_state();
    let trie_db = Arc::new(RocksTrieDB::new(
        path_state,
        config.executor.light,
        config.rocksdb.max_open_files,
    )?);

    // self private key
    let hex_privkey = hex::decode(config.privkey.as_string_trim0x()).map_err(MainError::FromHex)?;
    let my_privkey =
        Secp256k1PrivateKey::try_from(hex_privkey.as_ref()).map_err(MainError::Crypto)?;
    let my_pubkey = my_privkey.pub_key();
    let my_address = Address::from_pubkey_bytes(my_pubkey.to_bytes())?;

    // Get metadata
    let api_adapter = DefaultAPIAdapter::<ServiceExecutorFactory, _, _, _, _>::new(
        Arc::clone(&mempool),
        Arc::clone(&storage),
        Arc::clone(&trie_db),
        Arc::clone(&service_mapping),
    );

    let exec_resp = api_adapter
        .query_service(
            Context::new(),
            current_block.header.height,
            u64::max_value(),
            1,
            my_address.clone(),
            "metadata".to_string(),
            "get_metadata".to_string(),
            "".to_string(),
        )
        .await?;

    let metadata: Metadata =
        serde_json::from_str(&exec_resp.succeed_data).expect("Decode metadata failed!");

    // set args in mempool
    mempool.set_args(
        metadata.timeout_gap,
        metadata.cycles_limit,
        metadata.max_tx_size,
    );

    // register broadcast new transaction
    network_service.register_endpoint_handler(
        END_GOSSIP_NEW_TXS,
        Box::new(NewTxsHandler::new(Arc::clone(&mempool))),
    )?;

    // register pull txs from other node
    network_service.register_endpoint_handler(
        RPC_PULL_TXS,
        Box::new(PullTxsHandler::new(
            Arc::new(network_service.handle()),
            Arc::clone(&mempool),
        )),
    )?;
    network_service.register_rpc_response::<MsgPushTxs>(RPC_RESP_PULL_TXS)?;

    network_service.register_rpc_response::<MsgPushTxs>(RPC_RESP_PULL_TXS_SYNC)?;

    // Init Consensus
    let overlord_adapter = Arc::new(
        OverlordAdapter::<ServiceExecutorFactory, _, _, _, _, _, _>::new(
            &metadata.chain_id,
            &my_address,
            Arc::new(network_service.handle()),
            Arc::new(network_service.handle()),
            &mempool,
            &storage,
            &trie_db,
            &service_mapping,
        ),
    );

    let common_ref = metadata.common_ref.as_string();
    let key_pair = gen_keypair(Some(&config.privkey.as_string()), common_ref.clone());
    let wal_path = config.data_path_for_wal().to_str().unwrap().to_string();
    let overlord_adapter_clone = Arc::clone(&overlord_adapter);
    tokio::spawn(async move {
        OverlordServer::run(
            Context::new(),
            common_ref,
            key_pair.private_key,
            key_pair.public_key,
            key_pair.bls_public_key,
            my_address.as_bytes(),
            &overlord_adapter_clone,
            &wal_path,
        )
        .await;
    });

    // register consensus
    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_PROPOSAL,
        Box::new(SignedProposalHandler::new(Arc::clone(&overlord_adapter))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_PRE_VOTE,
        Box::new(SignedPreVoteHandler::new(Arc::clone(&overlord_adapter))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_PRE_COMMIT,
        Box::new(SignedPreCommitHandler::new(Arc::clone(&overlord_adapter))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_PRE_VOTE_QC,
        Box::new(PreVoteQCHandler::new(Arc::clone(&overlord_adapter))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_PRE_COMMIT_QC,
        Box::new(PreCommitQCHandler::new(Arc::clone(&overlord_adapter))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_CHOKE,
        Box::new(SignedChokeHandler::new(Arc::clone(&overlord_adapter))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_HEIGHT,
        Box::new(SignedHeightHandler::new(Arc::clone(&overlord_adapter))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_SYNC_REQUEST,
        Box::new(SyncRequestHandler::new(Arc::clone(&overlord_adapter))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_SYNC_RESPONSE,
        Box::new(SyncResponseHandler::new(Arc::clone(&overlord_adapter))),
    )?;

    // Run network
    tokio::spawn(network_service);

    // Init graphql
    let mut graphql_config = GraphQLConfig::default();
    graphql_config.listening_address = config.graphql.listening_address;
    graphql_config.graphql_uri = config.graphql.graphql_uri.clone();
    graphql_config.graphiql_uri = config.graphql.graphiql_uri.clone();
    if config.graphql.workers != 0 {
        graphql_config.workers = config.graphql.workers;
    }
    if config.graphql.maxconn != 0 {
        graphql_config.maxconn = config.graphql.maxconn;
    }
    if config.graphql.max_payload_size != 0 {
        graphql_config.max_payload_size = config.graphql.max_payload_size;
    }

    tokio::task::spawn_local(async move {
        let local = tokio::task::LocalSet::new();
        let actix_rt = actix_rt::System::run_in_tokio("muta-graphql", &local);
        tokio::task::spawn_local(actix_rt);

        core_api::start_graphql(graphql_config, api_adapter).await;
    });

    #[cfg(windows)]
    let _ = tokio::signal::ctrl_c().await;
    #[cfg(unix)]
    {
        let mut sigtun_int = os_impl::signal(os_impl::SignalKind::interrupt()).unwrap();
        let mut sigtun_term = os_impl::signal(os_impl::SignalKind::terminate()).unwrap();
        tokio::select! {
            _ = sigtun_int.recv() => {}
            _ = sigtun_term.recv() => {}
        }
    }

    Ok(())
}
