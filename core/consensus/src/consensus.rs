use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use creep::Context;
use overlord::types::{AggregatedVote, OverlordMsg, SignedProposal, SignedVote};
use overlord::{Codec, Consensus as OverlordConsensus, Crypto, Overlord, OverlordHandler};
use rlp::decode;

use protocol::traits::{
    Consensus, ConsensusAdapter, Gossip, MemPool, MemPoolAdapter, Storage, StorageAdapter,
};
use protocol::types::{Epoch, Proof, SignedTransaction, UserAddress};
use protocol::ProtocolResult;

use crate::adapter::OverlordConsensusAdapter;
use crate::{ConsensusError, MsgType};

/// Provide consensus
pub struct ConsensusProvider<
    CA: ConsensusAdapter,
    MA: MemPoolAdapter,
    SA: StorageAdapter,
    G: Gossip,
    M: MemPool<MA>,
    S: Storage<SA>,
    C: Codec,
    E: Codec,
    F: OverlordConsensus<C, E>,
    T: Crypto,
> {
    /// Overlord consensus protocol instance.
    overlord: Option<Overlord<C, E, F, T>>,
    /// An overlord consensus protocol handler.
    handler: OverlordHandler<C>,
    /// Supply necessary functions from other modules.
    adapter: CA,
    /// **TODO** to be changed into `Engine`
    overlord_adapter: OverlordConsensusAdapter<G, M, S, MA, SA>,

    mempool_adapter: PhantomData<MA>,
    storage_adapter: PhantomData<SA>,
}

#[async_trait]
impl<CA, MA, SA, G, M, S, C, E, F, T> Consensus<CA>
    for ConsensusProvider<CA, MA, SA, G, M, S, C, E, F, T>
where
    CA: ConsensusAdapter + 'static,
    MA: MemPoolAdapter + 'static,
    SA: StorageAdapter + 'static,
    G: Gossip + Send + Sync,
    M: MemPool<MA> + 'static,
    S: Storage<SA> + 'static,
    C: Codec + Send + Sync + 'static,
    E: Codec + Send + Sync + 'static,
    F: OverlordConsensus<C, E> + 'static,
    T: Crypto + Send + Sync + 'static,
{
    async fn set_proposal(&self, ctx: Context, proposal: Vec<u8>) -> ProtocolResult<()> {
        let signed_proposal: SignedProposal<C> =
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

impl<CA, MA, SA, G, M, S, C, E, F, T> ConsensusProvider<CA, MA, SA, G, M, S, C, E, F, T>
where
    CA: ConsensusAdapter + 'static,
    MA: MemPoolAdapter + 'static,
    SA: StorageAdapter + 'static,
    G: Gossip + Send + Sync,
    M: MemPool<MA> + 'static,
    S: Storage<SA> + 'static,
    C: Codec + Send + Sync + 'static,
    E: Codec + Send + Sync + 'static,
    F: OverlordConsensus<C, E> + 'static,
    T: Crypto + Send + Sync + 'static,
{
    pub fn new(
        address: UserAddress,
        cycle_limit: u64,
        overlord_adapter: F,
        crypto: T,
        consensus_adapter: CA,
        gossip_network: Arc<G>,
        menpool_adapter: Arc<M>,
        storage_adapter: Arc<S>,
    ) -> Self {
        let mut overlord = Overlord::new(address.as_bytes(), overlord_adapter, crypto);
        let overlord_handler = overlord.take_handler();

        ConsensusProvider {
            overlord:         Some(overlord),
            handler:          overlord_handler,
            adapter:          consensus_adapter,
            overlord_adapter: OverlordConsensusAdapter::new(
                address,
                cycle_limit,
                gossip_network,
                menpool_adapter,
                storage_adapter,
            ),

            mempool_adapter: PhantomData,
            storage_adapter: PhantomData,
        }
    }

    pub fn take_overlord(&mut self) -> Overlord<C, E, F, T> {
        assert!(self.overlord.is_some());
        self.overlord.take().unwrap()
    }
}
