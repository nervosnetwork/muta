use std::time::{Duration, Instant};

use futures::{compat::Future01CompatExt, future::TryFutureExt};
use tokio::timer::Delay;

use core_context::Context;
use core_crypto::{Crypto, CryptoTransform};
use core_runtime::{Consensus, Executor, FutConsensusResult, Storage, TransactionPool};
use core_serialization::{AsyncCodec, Proposal as SerProposal};
use core_types::{Block, Hash, Proof, SignedTransaction, Vote};

use crate::engine::Engine;
use crate::{ConsensusError, ConsensusResult, ProposalMessage, VoteMessage};

pub struct Solo<E, T, S, C>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
{
    engine:   Engine<E, T, S, C>,
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
        let proposal = self.engine.build_proposal(ctx.clone()).await?;
        let ser_proposal: SerProposal = proposal.clone().into();

        let encoded = AsyncCodec::encode(ser_proposal).await?;
        let proposal_hash = Hash::digest(&encoded);
        let signature = self.engine.sign_with_hash(&proposal_hash)?;
        let status = self.engine.get_status();

        let latest_proof = Proof {
            height:        proposal.height,
            round:         0,
            proposal_hash: proposal_hash.clone(),
            commits:       vec![Vote {
                address:   status.node_address,
                signature: signature.as_bytes().to_vec(),
            }],
        };

        self.engine
            .commit_block(ctx.clone(), proposal, latest_proof)
            .await?;

        Ok(())
    }

    pub async fn start(&self) -> ConsensusResult<()> {
        let interval = Duration::from_millis(self.interval);

        loop {
            let start_time = Instant::now();
            self.boom().await?;
            let now = Instant::now();
            let next = if now - start_time > interval {
                now
            } else {
                now + (interval - (now - start_time))
            };

            Delay::new(next)
                .compat()
                .map_err(|_| ConsensusError::Internal("internal".to_owned()))
                .await?;
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
    fn set_proposal(&self, _: Context, _: ProposalMessage) -> FutConsensusResult {
        unreachable!()
    }

    fn set_vote(&self, _: Context, _: VoteMessage) -> FutConsensusResult {
        unreachable!()
    }

    fn send_status(&self) -> FutConsensusResult {
        unreachable!()
    }

    fn insert_sync_block(
        &self,
        _: Context,
        _: Block,
        _: Vec<SignedTransaction>,
        _: Proof,
    ) -> FutConsensusResult {
        unreachable!()
    }
}
