use super::diagnostic::{
    TrustNewIntervalHandler, TrustTwinEventHandler, GOSSIP_TRUST_NEW_INTERVAL,
    GOSSIP_TRUST_TWIN_EVENT,
};
/// Almost same as src/default_start.rs, only remove graphql service.
use super::{config::Config, consts, error::MainError, memory_db::MemoryDB, Sync};
use crate::trust_metric_all::common;

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
use core_consensus::fixed_types::{FixedBlock, FixedProof, FixedSignedTxs};
use core_consensus::message::{
    ChokeMessageHandler, ProposalMessageHandler, PullBlockRpcHandler, PullProofRpcHandler,
    PullTxsRpcHandler, QCMessageHandler, RemoteHeightMessageHandler, VoteMessageHandler,
    BROADCAST_HEIGHT, END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_CHOKE,
    END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE, RPC_RESP_SYNC_PULL_BLOCK,
    RPC_RESP_SYNC_PULL_PROOF, RPC_RESP_SYNC_PULL_TXS, RPC_SYNC_PULL_BLOCK, RPC_SYNC_PULL_PROOF,
    RPC_SYNC_PULL_TXS,
};
use core_consensus::status::{CurrentConsensusStatus, StatusAgent};
use core_consensus::util::OverlordCrypto;
use core_consensus::{
    DurationConfig, Node, OverlordConsensus, OverlordConsensusAdapter, OverlordSynchronization,
    RichBlock, SignedTxsWAL,
};
use core_mempool::{
    DefaultMemPoolAdapter, HashMemPool, MsgPushTxs, NewTxsHandler, PullTxsHandler,
    END_GOSSIP_NEW_TXS, RPC_PULL_TXS, RPC_RESP_PULL_TXS,
};
use core_network::{DiagnosticEvent, NetworkConfig, NetworkService, PeerId, PeerIdExt};
use core_storage::{ImplStorage, StorageError};
use framework::executor::{ServiceExecutor, ServiceExecutorFactory};
use protocol::traits::{APIAdapter, Context, MemPool, Network, NodeInfo, ServiceMapping, Storage};
use protocol::types::{Address, Block, BlockHeader, Genesis, Hash, Metadata, Proof, Validator};
use protocol::{fixed_codec::FixedCodec, ProtocolResult};

pub async fn create_genesis<Mapping: 'static + ServiceMapping>(
    genesis: &Genesis,
    servive_mapping: Arc<Mapping>,
    db: MemoryDB,
) -> ProtocolResult<Block> {
    let metadata: Metadata =
        serde_json::from_str(genesis.get_payload("metadata")).expect("Decode metadata failed!");

    let validators: Vec<Validator> = metadata
        .verifier_list
        .iter()
        .map(|v| Validator {
            pub_key:        v.pub_key.decode(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect();

    // Read genesis.
    log::info!("Genesis data: {:?}", genesis);

    // Init Block db
    let storage = Arc::new(ImplStorage::new(Arc::new(db.clone())));

    match storage.get_latest_block(Context::new()).await {
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

    // Init genesis
    let genesis_state_root = ServiceExecutor::create_genesis(
        genesis.services.clone(),
        Arc::new(db),
        Arc::clone(&storage),
        servive_mapping,
    )?;

    // Build genesis block.
    let genesis_block_header = BlockHeader {
        chain_id: metadata.chain_id.clone(),
        height: 0,
        exec_height: 0,
        prev_hash: Hash::from_empty(),
        timestamp: genesis.timestamp,
        order_root: Hash::from_empty(),
        order_signed_transactions_hash: Hash::from_empty(),
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
    storage
        .insert_block(Context::new(), genesis_block.clone())
        .await?;
    storage
        .update_latest_proof(Context::new(), latest_proof)
        .await?;

    log::info!("The genesis block is created {:?}", genesis_block);
    Ok(genesis_block)
}

pub async fn start<Mapping: 'static + ServiceMapping>(
    config: Config,
    service_mapping: Arc<Mapping>,
    db: MemoryDB,
    sync: Sync,
) -> ProtocolResult<()> {
    log::info!("node starts");
    // Init Block db
    let storage = Arc::new(ImplStorage::new(Arc::new(db.clone())));

    // Init network
    let network_config = NetworkConfig::new()
        .max_connections(config.network.max_connected_peers)
        .allowlist_only(config.network.allowlist_only)
        .peer_trust_metric(
            consts::NETWORK_TRUST_METRIC_INTERVAL,
            config.network.trust_max_history_duration,
        )?
        .peer_soft_ban(consts::NETWORK_SOFT_BAND_DURATION)
        .peer_fatal_ban(config.network.fatal_ban_duration)
        .rpc_timeout(config.network.rpc_timeout)
        .ping_interval(consts::NETWORK_PING_INTERVAL)
        .selfcheck_interval(config.network.selfcheck_interval)
        .max_wait_streams(config.network.max_wait_streams)
        .max_frame_length(config.network.max_frame_length)
        .send_buffer_size(config.network.send_buffer_size)
        .write_timeout(config.network.write_timeout)
        .recv_buffer_size(config.network.recv_buffer_size);

    let network_privkey = config.privkey.as_string_trim0x();

    let mut bootstrap_pairs = vec![];
    if let Some(bootstrap) = &config.network.bootstraps {
        for bootstrap in bootstrap.iter() {
            bootstrap_pairs.push((bootstrap.peer_id.to_owned(), bootstrap.address.to_owned()));
        }
    }

    let allowlist = config.network.allowlist.clone().unwrap_or_default();

    let network_config = network_config
        .bootstraps(bootstrap_pairs)?
        .allowlist(allowlist)?
        .secio_keypair(network_privkey)?;
    let mut network_service = NetworkService::new(network_config);
    network_service
        .listen(config.network.listening_address)
        .await?;

    // Register diagnostic
    network_service.register_endpoint_handler(
        GOSSIP_TRUST_NEW_INTERVAL,
        Box::new(TrustNewIntervalHandler::new(
            sync.clone(),
            network_service.handle(),
        )),
    )?;
    network_service.register_endpoint_handler(
        GOSSIP_TRUST_TWIN_EVENT,
        Box::new(TrustTwinEventHandler(network_service.handle())),
    )?;

    let hook_fn = |sync: Sync| -> _ { Box::new(move |event: DiagnosticEvent| sync.emit(event)) };
    network_service.register_diagnostic_hook(hook_fn(sync.clone()));

    // Init mempool
    let current_block = storage.get_latest_block(Context::new()).await?;
    let mempool_adapter =
        DefaultMemPoolAdapter::<ServiceExecutorFactory, Secp256k1, _, _, _, _>::new(
            network_service.handle(),
            Arc::clone(&storage),
            Arc::new(db.clone()),
            Arc::clone(&service_mapping),
            config.mempool.broadcast_txs_size,
            config.mempool.broadcast_txs_interval,
        );
    let mempool = Arc::new(HashMemPool::new(consts::MEMPOOL_POOL_SIZE, mempool_adapter));

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
        Arc::new(db.clone()),
        Arc::clone(&service_mapping),
    );

    // Create full transactions wal
    let wal_path = common::tmp_dir()
        .to_str()
        .expect("wal path string")
        .to_string();
    let txs_wal = Arc::new(SignedTxsWAL::new(wal_path));

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

    // Init Consensus
    let validators: Vec<Validator> = metadata
        .verifier_list
        .iter()
        .map(|v| Validator {
            pub_key:        v.pub_key.decode(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect();

    let node_info = NodeInfo {
        chain_id:     metadata.chain_id.clone(),
        self_address: my_address.clone(),
        self_pub_key: my_pubkey.to_bytes(),
    };
    let current_header = &current_block.header;
    let block_hash = Hash::digest(current_block.header.encode_fixed()?);
    let current_height = current_block.header.height;
    let exec_height = current_block.header.exec_height;

    let current_consensus_status = CurrentConsensusStatus {
        cycles_price:                metadata.cycles_price,
        cycles_limit:                metadata.cycles_limit,
        latest_committed_height:     current_block.header.height,
        exec_height:                 current_block.header.exec_height,
        current_hash:                block_hash,
        latest_committed_state_root: current_header.state_root.clone(),
        list_confirm_root:           vec![],
        list_state_root:             vec![],
        list_receipt_root:           vec![],
        list_cycles_used:            vec![],
        current_proof:               current_header.proof.clone(),
        validators:                  validators.clone(),
        consensus_interval:          metadata.interval,
        propose_ratio:               metadata.propose_ratio,
        prevote_ratio:               metadata.prevote_ratio,
        precommit_ratio:             metadata.precommit_ratio,
        brake_ratio:                 metadata.brake_ratio,
        max_tx_size:                 metadata.max_tx_size,
        tx_num_limit:                metadata.tx_num_limit,
    };

    let consensus_interval = current_consensus_status.consensus_interval;
    let status_agent = StatusAgent::new(current_consensus_status);

    let mut bls_pub_keys = HashMap::new();
    for validator_extend in metadata.verifier_list.iter() {
        let address = validator_extend.pub_key.decode();
        let hex_pubkey = hex::decode(validator_extend.bls_pub_key.as_string_trim0x())
            .map_err(MainError::FromHex)?;
        let pub_key = BlsPublicKey::try_from(hex_pubkey.as_ref()).map_err(MainError::Crypto)?;
        bls_pub_keys.insert(address, pub_key);
    }

    let mut priv_key = Vec::new();
    priv_key.extend_from_slice(&[0u8; 16]);
    let mut tmp = hex::decode(config.privkey.as_string_trim0x()).unwrap();
    priv_key.append(&mut tmp);
    let bls_priv_key = BlsPrivateKey::try_from(priv_key.as_ref()).map_err(MainError::Crypto)?;

    let hex_common_ref =
        hex::decode(metadata.common_ref.as_string_trim0x()).map_err(MainError::FromHex)?;
    let common_ref: BlsCommonReference = std::str::from_utf8(hex_common_ref.as_ref())
        .map_err(MainError::Utf8)?
        .into();

    let crypto = Arc::new(OverlordCrypto::new(bls_priv_key, bls_pub_keys, common_ref));

    let mut consensus_adapter =
        OverlordConsensusAdapter::<ServiceExecutorFactory, _, _, _, _, _>::new(
            Arc::new(network_service.handle()),
            Arc::clone(&mempool),
            Arc::clone(&storage),
            Arc::new(db),
            Arc::clone(&service_mapping),
            status_agent.clone(),
            Arc::clone(&crypto),
        )?;

    let exec_demon = consensus_adapter.take_exec_demon();
    let consensus_adapter = Arc::new(consensus_adapter);

    let lock = Arc::new(Mutex::new(()));

    let overlord_consensus = Arc::new(OverlordConsensus::new(
        status_agent.clone(),
        node_info,
        Arc::clone(&crypto),
        Arc::clone(&txs_wal),
        Arc::clone(&consensus_adapter),
        Arc::clone(&lock),
    ));

    consensus_adapter.set_overlord_handler(overlord_consensus.get_overlord_handler());

    let synchronization = Arc::new(OverlordSynchronization::<_>::new(
        config.consensus.sync_txs_chunk_size,
        consensus_adapter,
        status_agent.clone(),
        crypto,
        lock,
    ));

    let peer_ids = metadata
        .verifier_list
        .iter()
        .map(|v| PeerId::from_pubkey_bytes(v.pub_key.decode()).map(PeerIdExt::into_bytes_ext))
        .collect::<Result<Vec<_>, _>>()?;

    network_service
        .handle()
        .tag_consensus(Context::new(), peer_ids)?;

    // Re-execute block from exec_height + 1 to current_height, so that init the
    // lost current status.
    log::info!("Re-execute from {} to {}", exec_height + 1, current_height);
    for height in exec_height + 1..=current_height {
        let block = storage
            .get_block(Context::new(), height)
            .await?
            .ok_or_else(|| StorageError::GetNone)?;
        let txs = storage
            .get_transactions(
                Context::new(),
                block.header.height,
                block.ordered_tx_hashes.clone(),
            )
            .await?
            .into_iter()
            .filter_map(|opt_stx| opt_stx)
            .collect::<Vec<_>>();
        if txs.len() != block.ordered_tx_hashes.len() {
            return Err(StorageError::GetNone.into());
        }
        let rich_block = RichBlock { block, txs };
        let _ = synchronization
            .exec_block(Context::new(), rich_block, status_agent.clone())
            .await?;
    }

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
        RPC_SYNC_PULL_PROOF,
        Box::new(PullProofRpcHandler::new(
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
    network_service.register_rpc_response::<FixedProof>(RPC_RESP_SYNC_PULL_PROOF)?;
    network_service.register_rpc_response::<FixedSignedTxs>(RPC_RESP_SYNC_PULL_TXS)?;

    // Run network
    tokio::spawn(network_service);
    sync.wait().await;

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
            address:        v.pub_key.clone(),
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

    let consensus_handle = tokio::spawn(async move {
        if let Err(e) = overlord_consensus
            .run(consensus_interval, authority_list, Some(timer_config))
            .await
        {
            log::error!("muta-consensus: {:?} error", e);
        }
    });

    exec_demon.run().await;
    let _ = consensus_handle.await;
    let _ = sync;

    Ok(())
}
