mod support;

use std::sync::Arc;

use bft_rs::{BftActuator, BftMsg, Node, Status as BftStatus};
use futures::prelude::{FutureExt, TryFutureExt};

use core_context::{CommonValue, Context};
use core_crypto::Crypto;
use core_pubsub::register::Register;
use core_runtime::{Executor, TransactionPool};
use core_storage::Storage;
use core_types::Hash;

use crate::bft::support::Support;
use crate::{
    Consensus, ConsensusError, ConsensusResult, Engine, FutConsensusResult, PorposalMessage,
    VoteMessage,
};

pub struct Bft<E, T, S, C>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
{
    bft_actuator: Arc<BftActuator>,
    support:      Arc<Support<E, T, S, C>>,
}

impl<E, T, S, C> Bft<E, T, S, C>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
{
    pub fn new(
        engine: Engine<E, T, S, C>,
        register: Register,
        wal_path: &str,
    ) -> ConsensusResult<Self> {
        let status = engine.get_status()?;
        let support = Support::new(engine, register)?;
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
            bft_actuator: Arc::new(bft_actuator),
            support,
        })
    }
}

impl<E, T, S, C> Consensus for Bft<E, T, S, C>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
{
    fn set_proposal(&self, ctx: Context, msg: PorposalMessage) -> FutConsensusResult<()> {
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
}

impl<E, T, S, C> Clone for Bft<E, T, S, C>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
{
    fn clone(&self) -> Self {
        Self {
            bft_actuator: Arc::clone(&self.bft_actuator),
            support:      Arc::clone(&self.support),
        }
    }
}
