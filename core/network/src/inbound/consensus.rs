use std::clone::Clone;
use std::sync::Arc;

use core_context::Context;
use core_network_message::consensus::{Proposal, Vote};
use core_network_message::{Codec, Method};
use core_runtime::Consensus;

use crate::Error;

pub struct ConsensusReactor<C> {
    consensus: Arc<C>,
}

impl<C> Clone for ConsensusReactor<C> {
    fn clone(&self) -> Self {
        ConsensusReactor {
            consensus: Arc::clone(&self.consensus),
        }
    }
}

impl<C> ConsensusReactor<C>
where
    C: Consensus + 'static,
{
    pub fn new(consensus: Arc<C>) -> Self {
        ConsensusReactor { consensus }
    }

    pub async fn react(&self, ctx: Context, method: Method, data: Vec<u8>) -> Result<(), Error> {
        match method {
            Method::Proposal => self.handle_proposal(ctx, data).await?,
            Method::Vote => self.handle_vote(ctx, data).await?,
            _ => Err(Error::UnknownMethod(method.to_u32()))?,
        };

        Ok(())
    }

    pub async fn handle_proposal(&self, ctx: Context, msg: Vec<u8>) -> Result<(), Error> {
        let proposal = <Proposal as Codec>::decode(msg.as_slice())?;

        self.consensus.set_proposal(ctx, proposal.des()).await?;

        Ok(())
    }

    pub async fn handle_vote(&self, ctx: Context, msg: Vec<u8>) -> Result<(), Error> {
        let vote = <Vote as Codec>::decode(msg.as_slice())?;

        self.consensus.set_vote(ctx, vote.des()).await?;

        Ok(())
    }
}
