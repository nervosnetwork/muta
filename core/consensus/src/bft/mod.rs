mod support;

use std::sync::Arc;

use bft_rs::{BftActuator, BftMsg, Node, Status as BftStatus};
use futures::prelude::{FutureExt, TryFutureExt};

use core_context::{CommonValue, Context};
use core_crypto::Crypto;
use core_runtime::network::Consensus as Network;
use core_runtime::{
    Consensus, ConsensusError, Executor, FutConsensusResult, Storage, TransactionPool,
};
use core_types::{Block, Hash, Proof, SignedTransaction};

use crate::bft::support::Support;
use crate::{ConsensusResult, Engine, ProposalMessage, VoteMessage};

pub struct Bft<E, T, S, C, N>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    N: Network + 'static,
{
    engine:       Arc<Engine<E, T, S, C>>,
    bft_actuator: Arc<BftActuator>,
    support:      Arc<Support<E, T, S, C, N>>,
}

impl<E, T, S, C, N> Bft<E, T, S, C, N>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    N: Network + 'static,
{
    pub fn new(
        engine: Arc<Engine<E, T, S, C>>,
        network: N,
        wal_path: &str,
    ) -> ConsensusResult<Self> {
        let status = engine.get_status();
        let support = Support::new(Arc::clone(&engine), network)?;
        let support = Arc::new(support);

        let bft_actuator = BftActuator::new(
            Arc::clone(&support),
            status.node_address.as_bytes().to_vec(),
            wal_path,
        );

        bft_actuator.send(BftMsg::Status(BftStatus {
            height:         status.height,
            interval:       Some(status.interval),
            authority_list: status
                .verifier_list
                .iter()
                .map(|a| Node::set_address(a.as_bytes().to_vec()))
                .collect(),
        }))?;

        Ok(Self {
            engine,
            bft_actuator: Arc::new(bft_actuator),
            support,
        })
    }
}

impl<E, T, S, C, N> Consensus for Bft<E, T, S, C, N>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    N: Network + 'static,
{
    fn send_status(&self) -> FutConsensusResult<()> {
        let bft = self.clone();
        let fut = async move {
            let status = bft.engine.get_status();
            bft.bft_actuator.send(BftMsg::Status(BftStatus {
                height:         status.height,
                interval:       Some(status.interval),
                authority_list: status
                    .verifier_list
                    .iter()
                    .map(|a| Node::set_address(a.as_bytes().to_vec()))
                    .collect(),
            }))?;
            Ok(())
        };
        Box::new(fut.boxed().compat())
    }

    fn set_proposal(&self, ctx: Context, msg: ProposalMessage) -> FutConsensusResult<()> {
        let bft = self.clone();

        let fut = async move {
            let hash = Hash::digest(&msg);
            let session_id = ctx.p2p_session_id().ok_or_else(|| {
                ConsensusError::InvalidProposal("session id cannot be empty".to_owned())
            })?;

            bft.support.insert_proposal_origin(hash, session_id)?;
            bft.bft_actuator.send(BftMsg::Proposal(msg))?;
            Ok(())
        };

        Box::new(fut.boxed().compat())
    }

    fn set_vote(&self, _: Context, msg: VoteMessage) -> FutConsensusResult<()> {
        let bft = self.clone();

        let fut = async move {
            bft.bft_actuator.send(BftMsg::Vote(msg))?;
            Ok(())
        };

        Box::new(fut.boxed().compat())
    }

    fn insert_sync_block(
        &self,
        ctx: Context,
        block: Block,
        stxs: Vec<SignedTransaction>,
        proof: Proof,
    ) -> FutConsensusResult<()> {
        let bft = self.clone();

        let fut = async move {
            bft.engine
                .insert_sync_block(ctx, block, stxs, proof)
                .await?;
            Ok(())
        };

        Box::new(fut.boxed().compat())
    }
}

impl<E, T, S, C, N> Clone for Bft<E, T, S, C, N>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    N: Network + 'static,
{
    fn clone(&self) -> Self {
        Self {
            engine:       Arc::clone(&self.engine),
            bft_actuator: Arc::clone(&self.bft_actuator),
            support:      Arc::clone(&self.support),
        }
    }
}
