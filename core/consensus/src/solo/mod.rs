use std::time::{Duration, Instant};

use futures::{compat::Future01CompatExt, future::TryFutureExt};
use tokio::timer::Delay;

use core_context::Context;
use core_crypto::{Crypto, CryptoTransform};
use core_runtime::{Executor, TransactionPool};
use core_serialization::{AsyncCodec, Proposal as SerProposal};
use core_storage::Storage;
use core_types::{Hash, Proof, Vote};

use crate::engine::Engine;
use crate::{
    Consensus, ConsensusError, ConsensusResult, FutConsensusResult, ProposalMessage, VoteMessage,
};

pub struct Solo<E, T, S, C>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
{
    engine: Engine<E, T, S, C>,
    interval: u64,
}

impl<E, T, S, C> Solo<E, T, S, C>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
{
    pub fn new(engine: Engine<E, T, S, C>, interval: u64) -> Result<Self, ConsensusError> {
        Ok(Self { engine, interval })
    }

    async fn boom(&self) -> ConsensusResult<()> {
        let ctx = Context::new();
        let proposal = await!(self.engine.build_proposal(ctx.clone()))?;
        let ser_proposal: SerProposal = proposal.clone().into();

        let encoded = await!(AsyncCodec::encode(ser_proposal))?;
        let proposal_hash = Hash::digest(&encoded);
        let signature = self.engine.sign_with_hash(&proposal_hash)?;
        let status = self.engine.get_status()?;

        let latest_proof = Proof {
            height: proposal.height,
            round: 0,
            proposal_hash: proposal_hash.clone(),
            commits: vec![Vote {
                address: status.node_address,
                signature: signature.as_bytes().to_vec(),
            }],
        };

        await!(self
            .engine
            .commit_block(ctx.clone(), proposal, latest_proof))?;
        Ok(())
    }

    pub async fn start(&self) -> ConsensusResult<()> {
        let interval = Duration::from_millis(self.interval);

        loop {
            let start_time = Instant::now();
            await!(self.boom())?;
            let now = Instant::now();
            let next = if now - start_time > interval {
                now
            } else {
                now + (interval - (now - start_time))
            };
            await!(Delay::new(next)
                .compat()
                .map_err(|_| ConsensusError::Internal("internal".to_owned())))?;
        }
    }
}

impl<E, T, S, C> Consensus for Solo<E, T, S, C>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
{
    fn set_proposal(&self, _: Context, _: ProposalMessage) -> FutConsensusResult<()> {
        unreachable!()
    }

    fn set_vote(&self, _: Context, _: VoteMessage) -> FutConsensusResult<()> {
        unreachable!()
    }
}
