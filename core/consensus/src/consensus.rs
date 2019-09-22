use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use creep::Context;
use overlord::types::{AggregatedVote, Node, OverlordMsg, SignedProposal, SignedVote, Status};
use overlord::{Overlord, OverlordHandler};
use rlp::decode;

use common_crypto::{Secp256k1PrivateKey, Secp256k1PublicKey};

use protocol::traits::{Consensus, Gossip, MemPool, MemPoolAdapter, Storage, StorageAdapter};
use protocol::types::{Epoch, Hash, Proof, SignedTransaction, UserAddress, Validator};
use protocol::ProtocolResult;

use crate::adapter::OverlordConsensusAdapter;
use crate::engine::ConsensusEngine;
use crate::fixed_types::{FixedPill, FixedSignedTxs};
use crate::util::OverlordCrypto;
use crate::{ConsensusError, MsgType};

pub type OverlordRuntime<G, M, S, MA, SA> =
    Overlord<FixedPill, FixedSignedTxs, ConsensusEngine<G, M, S, MA, SA>, OverlordCrypto>;

/// Provide consensus
pub struct ConsensusProvider<
    MA: MemPoolAdapter + 'static,
    SA: StorageAdapter + 'static,
    G: Gossip + Send + Sync,
    M: MemPool<MA>,
    S: Storage<SA>,
> {
    /// Overlord consensus protocol instance.
    overlord: Option<OverlordRuntime<G, M, S, MA, SA>>,
    /// An overlord consensus protocol handler.
    handler: OverlordHandler<FixedPill>,

    mempool_adapter: PhantomData<MA>,
    storage_adapter: PhantomData<SA>,
}

#[async_trait]
impl<MA, SA, G, M, S> Consensus<OverlordConsensusAdapter<G, M, S, MA, SA>>
    for ConsensusProvider<MA, SA, G, M, S>
where
    MA: MemPoolAdapter + 'static,
    SA: StorageAdapter + 'static,
    G: Gossip + Send + Sync,
    M: MemPool<MA> + 'static,
    S: Storage<SA> + 'static,
{
    async fn set_proposal(&self, ctx: Context, proposal: Vec<u8>) -> ProtocolResult<()> {
        let signed_proposal: SignedProposal<FixedPill> =
            decode(&proposal).map_err(|_| ConsensusError::DecodeErr(MsgType::SignedProposal))?;
        self.handler
            .send_msg(ctx, OverlordMsg::SignedProposal(signed_proposal))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

    async fn set_vote(&self, ctx: Context, vote: Vec<u8>) -> ProtocolResult<()> {
        let signed_vote: SignedVote =
            decode(&vote).map_err(|_| ConsensusError::DecodeErr(MsgType::SignedVote))?;
        self.handler
            .send_msg(ctx, OverlordMsg::SignedVote(signed_vote))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

    async fn set_qc(&self, ctx: Context, qc: Vec<u8>) -> ProtocolResult<()> {
        let aggregated_vote: AggregatedVote =
            decode(&qc).map_err(|_| ConsensusError::DecodeErr(MsgType::AggregateVote))?;
        self.handler
            .send_msg(ctx, OverlordMsg::AggregatedVote(aggregated_vote))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

    async fn update_epoch(
        &self,
        _ctx: Context,
        _epoch: Epoch,
        _signed_txs: Vec<SignedTransaction>,
        _proof: Proof,
    ) -> ProtocolResult<()> {
        Ok(())
    }
}

impl<MA, SA, G, M, S> ConsensusProvider<MA, SA, G, M, S>
where
    MA: MemPoolAdapter + 'static,
    SA: StorageAdapter + 'static,
    G: Gossip + Send + Sync + 'static,
    M: MemPool<MA> + 'static,
    S: Storage<SA> + 'static,
{
    pub fn new(
        init_epoch_id: u64,
        chain_id: Hash,
        address: UserAddress,
        cycle_limit: u64,
        validators: Vec<Validator>,
        pub_key: Secp256k1PublicKey,
        priv_key: Secp256k1PrivateKey,
        gossip_network: Arc<G>,
        menpool_adapter: Arc<M>,
        storage_adapter: Arc<S>,
    ) -> Self {
        let engine = ConsensusEngine::new(
            chain_id,
            address.clone(),
            cycle_limit,
            validators.clone(),
            gossip_network,
            menpool_adapter,
            storage_adapter,
        );

        let crypto = OverlordCrypto::new(pub_key, priv_key);

        let mut overlord = Overlord::new(address.as_bytes(), engine, crypto);
        let overlord_handler = overlord.take_handler();
        overlord_handler
            .send_msg(
                Context::new(),
                OverlordMsg::RichStatus(handle_genesis(init_epoch_id, validators)),
            )
            .unwrap();

        ConsensusProvider {
            overlord:        Some(overlord),
            handler:         overlord_handler,
            mempool_adapter: PhantomData,
            storage_adapter: PhantomData,
        }
    }

    pub fn take_overlord(&mut self) -> OverlordRuntime<G, M, S, MA, SA> {
        assert!(self.overlord.is_some());
        self.overlord.take().unwrap()
    }
}

fn handle_genesis(epoch_id: u64, validators: Vec<Validator>) -> Status {
    let mut authority_list = validators
        .into_iter()
        .map(|v| Node {
            address:        v.address.as_bytes(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect::<Vec<_>>();

    authority_list.sort();
    Status {
        epoch_id,
        interval: None,
        authority_list,
    }
}
