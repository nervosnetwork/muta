use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

use bytes::Bytes;
use futures::lock::Mutex;

use common_crypto::{
    BlsCommonReference, BlsPrivateKey, BlsPublicKey, PublicKey, Secp256k1, Secp256k1PrivateKey,
    ToPublicKey,
};
use core_api::adapter::DefaultAPIAdapter;
use core_api::config::GraphQLConfig;
use core_consensus::fixed_types::{FixedBlock, FixedSignedTxs};
use core_consensus::message::{
    ChokeMessageHandler, ProposalMessageHandler, PullBlockRpcHandler, PullTxsRpcHandler,
    QCMessageHandler, RemoteHeightMessageHandler, VoteMessageHandler, BROADCAST_HEIGHT,
    END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_CHOKE, END_GOSSIP_SIGNED_PROPOSAL,
    END_GOSSIP_SIGNED_VOTE, RPC_RESP_SYNC_PULL_BLOCK, RPC_RESP_SYNC_PULL_TXS, RPC_SYNC_PULL_BLOCK,
    RPC_SYNC_PULL_TXS,
};
use core_consensus::status::{CurrentConsensusStatus, StatusAgent};
use core_consensus::{
    DurationConfig, Node, OverlordConsensus, OverlordConsensusAdapter, OverlordSynchronization,
    WalInfoQueue,
};
use core_mempool::{
    DefaultMemPoolAdapter, HashMemPool, MsgPushTxs, NewTxsHandler, PullTxsHandler,
    END_GOSSIP_NEW_TXS, RPC_PULL_TXS, RPC_RESP_PULL_TXS,
};
use core_network::{NetworkConfig, NetworkService};
use core_storage::{adapter::rocks::RocksAdapter, ImplStorage};
use framework::binding::state::RocksTrieDB;
use framework::executor::{ServiceExecutor, ServiceExecutorFactory};
use protocol::traits::{APIAdapter, Context, MemPool, NodeInfo, ServiceMapping, Storage};
use protocol::types::{Address, Block, BlockHeader, Genesis, Hash, Metadata, Proof, Validator};
use protocol::{fixed_codec::FixedCodec, ProtocolResult};

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
    let rocks_adapter = Arc::new(RocksAdapter::new(path_block)?);
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
    let trie_db = Arc::new(RocksTrieDB::new(path_state, config.executor.light)?);

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
        proposer: Address::from_hex("0000000000000000000000000000000000000000")?,
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
    // Init Block db
    let path_block = config.data_path_for_block();
    log::info!("Data path for block: {:?}", path_block);
    let rocks_adapter = Arc::new(RocksAdapter::new(path_block)?);
    let storage = Arc::new(ImplStorage::new(Arc::clone(&rocks_adapter)));

    // Init network
    let network_config = NetworkConfig::new()
        .rpc_timeout(config.network.rpc_timeout.clone())
        .selfcheck_interval(config.network.selfcheck_interval.clone())
        .max_wait_streams(config.network.max_wait_streams)
        .max_frame_length(config.network.max_frame_length.clone())
        .send_buffer_size(config.network.send_buffer_size.clone())
        .write_timeout(config.network.write_timeout)
        .recv_buffer_size(config.network.recv_buffer_size.clone());

    let network_privkey = config.privkey.clone();

    let mut bootstrap_pairs = vec![];
    if let Some(bootstrap) = &config.network.bootstraps {
        for bootstrap in bootstrap.iter() {
            bootstrap_pairs.push((bootstrap.pubkey.to_owned(), bootstrap.address.to_owned()));
        }
    }

    let network_config = network_config
        .bootstraps(bootstrap_pairs)?
        .secio_keypair(network_privkey)?;
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
    let trie_db = Arc::new(RocksTrieDB::new(path_state, config.executor.light)?);

    // self private key
    let hex_privkey = hex::decode(config.privkey.clone()).map_err(MainError::FromHex)?;
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

    let exec_resp = futures::executor::block_on(api_adapter.query_service(
        Context::new(),
        current_block.header.height,
        u64::max_value(),
        1,
        my_address.clone(),
        "metadata".to_string(),
        "get_metadata".to_string(),
        "".to_string(),
    ))?;

    let metadata: Metadata = serde_json::from_str(&exec_resp.ret).expect("Decode metadata failed!");

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

    // Init Consensus
    let validators: Vec<Validator> = metadata
        .verifier_list
        .iter()
        .map(|v| Validator {
            address:        v.address.clone(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect();

    let node_info = NodeInfo {
        chain_id:     metadata.chain_id.clone(),
        self_address: my_address.clone(),
    };
    let current_header = &current_block.header;
    let prevhash = Hash::digest(current_block.encode_fixed()?);

    let current_consensus_status = CurrentConsensusStatus {
        cycles_price:       metadata.cycles_price,
        cycles_limit:       metadata.cycles_limit,
        height:             current_block.header.height + 1,
        exec_height:        current_block.header.height,
        prev_hash:          prevhash,
        latest_state_root:  current_header.state_root.clone(),
        logs_bloom:         vec![],
        confirm_root:       vec![],
        state_root:         vec![current_header.state_root.clone()],
        receipt_root:       vec![],
        cycles_used:        vec![],
        proof:              current_header.proof.clone(),
        validators:         validators.clone(),
        consensus_interval: metadata.interval,
        propose_ratio:      metadata.propose_ratio,
        prevote_ratio:      metadata.prevote_ratio,
        precommit_ratio:    metadata.precommit_ratio,
        brake_ratio:        metadata.brake_ratio,
        max_tx_size:        metadata.max_tx_size,
        tx_num_limit:       metadata.tx_num_limit,
    };

    let consensus_interval = current_consensus_status.consensus_interval;
    let status_agent = StatusAgent::new(current_consensus_status);

    let mut bls_pub_keys = HashMap::new();
    for validator_extend in metadata.verifier_list.iter() {
        let address = validator_extend.address.as_bytes();
        let hex_pubkey =
            hex::decode(validator_extend.bls_pub_key.clone()).map_err(MainError::FromHex)?;
        let pub_key = BlsPublicKey::try_from(hex_pubkey.as_ref()).map_err(MainError::Crypto)?;
        bls_pub_keys.insert(address, pub_key);
    }

    let mut priv_key = Vec::new();
    priv_key.extend_from_slice(&[0u8; 16]);
    let mut tmp = hex::decode(config.privkey.clone()).unwrap();
    priv_key.append(&mut tmp);
    let bls_priv_key = BlsPrivateKey::try_from(priv_key.as_ref()).map_err(MainError::Crypto)?;

    let hex_common_ref = hex::decode(metadata.common_ref.as_str()).map_err(MainError::FromHex)?;
    let common_ref: BlsCommonReference = std::str::from_utf8(hex_common_ref.as_ref())
        .map_err(MainError::Utf8)?
        .into();

    core_consensus::trace::init_tracer(my_address.as_hex())?;

    let exec_wal = match storage.load_exec_queue_wal().await {
        Ok(bytes) => rlp::decode(bytes.as_ref()).unwrap(),
        Err(_) => WalInfoQueue::new(),
    };

    let mut consensus_adapter =
        OverlordConsensusAdapter::<ServiceExecutorFactory, _, _, _, _, _, _>::new(
            Arc::new(network_service.handle()),
            Arc::new(network_service.handle()),
            Arc::clone(&mempool),
            Arc::clone(&storage),
            Arc::clone(&trie_db),
            Arc::clone(&service_mapping),
            status_agent.clone(),
            exec_wal,
        )?;

    let exec_demon = consensus_adapter.take_exec_demon();
    let consensus_adapter = Arc::new(consensus_adapter);

    let lock = Arc::new(Mutex::new(()));
    let overlord_consensus = Arc::new(OverlordConsensus::new(
        status_agent.clone(),
        node_info,
        bls_pub_keys,
        bls_priv_key,
        common_ref,
        Arc::clone(&consensus_adapter),
        Arc::clone(&lock),
    ));

    consensus_adapter.set_overlord_handler(overlord_consensus.get_overlord_handler());

    let synchronization = Arc::new(OverlordSynchronization::new(
        consensus_adapter,
        status_agent,
        lock,
    ));

    // register consensus
    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_PROPOSAL,
        Box::new(ProposalMessageHandler::new(Arc::clone(&overlord_consensus))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_AGGREGATED_VOTE,
        Box::new(QCMessageHandler::new(Arc::clone(&overlord_consensus))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_VOTE,
        Box::new(VoteMessageHandler::new(Arc::clone(&overlord_consensus))),
    )?;
    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_CHOKE,
        Box::new(ChokeMessageHandler::new(Arc::clone(&overlord_consensus))),
    )?;
    network_service.register_endpoint_handler(
        BROADCAST_HEIGHT,
        Box::new(RemoteHeightMessageHandler::new(Arc::clone(
            &synchronization,
        ))),
    )?;
    network_service.register_endpoint_handler(
        RPC_SYNC_PULL_BLOCK,
        Box::new(PullBlockRpcHandler::new(
            Arc::new(network_service.handle()),
            Arc::clone(&storage),
        )),
    )?;
    network_service.register_endpoint_handler(
        RPC_SYNC_PULL_TXS,
        Box::new(PullTxsRpcHandler::new(
            Arc::new(network_service.handle()),
            Arc::clone(&storage),
        )),
    )?;
    network_service.register_rpc_response::<FixedBlock>(RPC_RESP_SYNC_PULL_BLOCK)?;
    network_service.register_rpc_response::<FixedSignedTxs>(RPC_RESP_SYNC_PULL_TXS)?;

    // Run network
    tokio::spawn(network_service);
    // Run sync
    tokio::spawn(async move {
        if let Err(e) = synchronization.polling_broadcast().await {
            log::error!("synchronization: {:?}", e);
        }
    });

    // Run consensus
    let authority_list = validators
        .iter()
        .map(|v| Node {
            address:        v.address.as_bytes(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect::<Vec<_>>();

    let timer_config = DurationConfig {
        propose_ratio:   metadata.propose_ratio,
        prevote_ratio:   metadata.prevote_ratio,
        precommit_ratio: metadata.precommit_ratio,
        brake_ratio:     metadata.brake_ratio,
    };

    tokio::spawn(async move {
        if let Err(e) = overlord_consensus
            .run(consensus_interval, authority_list, Some(timer_config))
            .await
        {
            log::error!("muta-consensus: {:?} error", e);
        }
    });

    tokio::spawn(async move {
        futures::executor::block_on(exec_demon.run());
    });

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

    let local = tokio::task::LocalSet::new();
    let actix_rt = actix_rt::System::run_in_tokio("muta-graphql", &local);
    tokio::spawn(async move {
        actix_rt.await.unwrap();
    });

    core_api::start_graphql(graphql_config, api_adapter).await;
    Ok(())
}
