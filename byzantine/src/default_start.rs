use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

use bytes::Bytes;
use common_crypto::{
    BlsCommonReference, BlsPrivateKey, BlsPublicKey, PublicKey, Secp256k1PrivateKey, ToPublicKey,
    UncompressedPublicKey,
};
use futures::channel::mpsc::unbounded;
use futures::future;
#[cfg(unix)]
use tokio::signal::unix::{self as os_impl};

use core_consensus::message::{
    BROADCAST_HEIGHT, END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_CHOKE,
    END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE,
};
use core_consensus::util::OverlordCrypto;
use core_mempool::{MsgPushTxs, END_GOSSIP_NEW_TXS, RPC_PULL_TXS, RPC_RESP_PULL_TXS};
use core_network::{NetworkConfig, NetworkService, PeerId, PeerIdExt};
use protocol::traits::{Context, Network};
use protocol::types::{Address, Genesis, Metadata, Validator};
use protocol::ProtocolResult;

use crate::commander::Commander;
use crate::config::{Config, Generators};
use crate::message::{
    ChokeMessageHandler, NewTxsHandler, ProposalMessageHandler, PullTxsHandler, QCMessageHandler,
    RemoteHeightMessageHandler, VoteMessageHandler,
};
use crate::worker::Worker;

pub async fn start(config: Config, genesis: Genesis, generators: Generators) -> ProtocolResult<()> {
    log::info!("byzantine node starts");

    // Init network
    let network_config = NetworkConfig::new()
        .max_connections(config.network.max_connected_peers)?
        .same_ip_conn_limit(config.network.same_ip_conn_limit)
        .inbound_conn_limit(config.network.inbound_conn_limit)?
        .allowlist_only(config.network.allowlist_only)
        .peer_trust_metric(
            config.network.trust_interval_duration,
            config.network.trust_max_history_duration,
        )?
        .peer_soft_ban(config.network.soft_ban_duration)
        .peer_fatal_ban(config.network.fatal_ban_duration)
        .rpc_timeout(config.network.rpc_timeout)
        .ping_interval(config.network.ping_interval)
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

    // self private key
    let hex_privkey =
        hex::decode(config.privkey.as_string_trim0x()).expect("decode privkey error!");
    let my_privkey =
        Secp256k1PrivateKey::try_from(hex_privkey.as_ref()).expect("get privkey failed!");
    let my_pubkey = my_privkey.pub_key();
    let my_address = Address::from_pubkey_bytes(my_pubkey.to_uncompressed_bytes())?;

    // get pub_key_list
    let metadata: Metadata =
        serde_json::from_str(genesis.get_payload("metadata")).expect("Decode metadata failed!");
    let pub_key_list: Vec<Bytes> = metadata
        .verifier_list
        .iter()
        .map(|v| v.pub_key.decode())
        .filter(|addr| addr != &my_pubkey.to_bytes())
        .collect();
    let validators: Vec<Validator> = metadata
        .verifier_list
        .iter()
        .map(|v| Validator {
            pub_key:        v.pub_key.decode(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect();

    assert_ne!(
        pub_key_list.len(),
        0,
        "It's meaningless to test a system contains only one node which is a byzantine node"
    );

    // get crypto
    let mut bls_pub_keys = HashMap::new();
    for validator_extend in metadata.verifier_list.iter() {
        let address = validator_extend.pub_key.decode();
        let hex_pubkey = hex::decode(validator_extend.bls_pub_key.as_string_trim0x())
            .expect("decode pubkey failed");
        let pub_key =
            BlsPublicKey::try_from(hex_pubkey.as_ref()).expect("try into BlsPublicKey failed");
        bls_pub_keys.insert(address, pub_key);
    }

    let mut priv_key = Vec::new();
    priv_key.extend_from_slice(&[0u8; 16]);
    let mut tmp = hex::decode(config.privkey.as_string_trim0x()).unwrap();
    priv_key.append(&mut tmp);
    let bls_priv_key =
        BlsPrivateKey::try_from(priv_key.as_ref()).expect("try into BlsPrivateKey failed");

    let hex_common_ref =
        hex::decode(metadata.common_ref.as_string_trim0x()).expect("decode common ref failed");
    let common_ref: BlsCommonReference = std::str::from_utf8(hex_common_ref.as_ref())
        .expect("transfer common_ref failed")
        .into();
    let crypto = OverlordCrypto::new(bls_priv_key, bls_pub_keys, common_ref);

    let (network_tx, network_rx) = unbounded();
    let (worker_tx, worker_rx) = unbounded();

    // set chain id in network
    network_service.set_chain_id(metadata.chain_id.clone());

    let peer_ids = metadata
        .verifier_list
        .iter()
        .map(|v| PeerId::from_pubkey_bytes(v.pub_key.decode()).map(PeerIdExt::into_bytes_ext))
        .collect::<Result<Vec<_>, _>>()?;

    network_service
        .handle()
        .tag_consensus(Context::new(), peer_ids)?;

    // register broadcast new transaction
    network_service
        .register_endpoint_handler(END_GOSSIP_NEW_TXS, NewTxsHandler::new(network_tx.clone()))?;

    // register pull txs from other node
    network_service
        .register_endpoint_handler(RPC_PULL_TXS, PullTxsHandler::new(network_tx.clone()))?;
    network_service.register_rpc_response::<MsgPushTxs>(RPC_RESP_PULL_TXS)?;

    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_PROPOSAL,
        ProposalMessageHandler::new(network_tx.clone()),
    )?;

    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_VOTE,
        VoteMessageHandler::new(network_tx.clone()),
    )?;

    network_service.register_endpoint_handler(
        END_GOSSIP_AGGREGATED_VOTE,
        QCMessageHandler::new(network_tx.clone()),
    )?;

    network_service.register_endpoint_handler(
        END_GOSSIP_SIGNED_CHOKE,
        ChokeMessageHandler::new(network_tx.clone()),
    )?;

    network_service.register_endpoint_handler(
        BROADCAST_HEIGHT,
        RemoteHeightMessageHandler::new(network_tx.clone()),
    )?;

    let commander = Commander::new(generators, pub_key_list, worker_tx, network_rx);
    let worker = Worker::new(
        my_address,
        my_pubkey.to_bytes(),
        metadata,
        validators,
        crypto,
        Arc::new(network_service.handle()),
        worker_rx,
    );

    // Run network
    tokio::spawn(network_service);

    // Run worker
    tokio::spawn(async move {
        worker.run().await;
    });

    // run commander
    let (abortable_demon, abort_handle) = future::abortable(commander.run());
    let exec_handler = tokio::task::spawn_local(abortable_demon);
    let ctrl_c_handler = tokio::task::spawn_local(async {
        #[cfg(windows)]
        let _ = tokio::signal::ctrl_c().await;
        #[cfg(unix)]
        {
            let mut sigtun_int = os_impl::signal(os_impl::SignalKind::interrupt()).unwrap();
            let mut sigtun_term = os_impl::signal(os_impl::SignalKind::terminate()).unwrap();
            tokio::select! {
                _ = sigtun_int.recv() => {}
                _ = sigtun_term.recv() => {}
            };
        }
    });

    tokio::select! {
        _ = exec_handler =>{log::error!("exec_daemon is down, quit.")},
        _ = ctrl_c_handler =>{log::info!("ctrl + c is pressed, quit.")},
    };
    abort_handle.abort();

    Ok(())
}
