use std::clone::Clone;
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::prelude::TryFutureExt;

use core_context::Context;
use core_network_message::consensus::{Proposal, Vote};
use core_network_message::{Codec, Method};
use core_runtime::Consensus;

use crate::inbound::{FutReactResult, Reactor};
use crate::Error;

pub type FutConsResult = BoxFuture<'static, Result<(), Error>>;

pub trait InboundConsensus: Send + Sync {
    fn set_proposal(&self, ctx: Context, msg: Vec<u8>) -> FutConsResult;

    fn set_vote(&self, ctx: Context, msg: Vec<u8>) -> FutConsResult;
}

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
    C: InboundConsensus + 'static,
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

impl<C> InboundConsensus for C
where
    C: Consensus,
{
    fn set_proposal(&self, ctx: Context, msg: Vec<u8>) -> FutConsResult {
        Box::pin(self.set_proposal(ctx, msg).err_into())
    }

    fn set_vote(&self, ctx: Context, msg: Vec<u8>) -> FutConsResult {
        Box::pin(self.set_vote(ctx, msg).err_into())
    }
}

impl<C> Reactor for ConsensusReactor<C>
where
    C: InboundConsensus + 'static,
{
    fn react(&self, ctx: Context, method: Method, data: Vec<u8>) -> FutReactResult {
        let reactor = self.clone();

        Box::pin(async move { reactor.react(ctx, method, data).await })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    use futures::executor::block_on;
    use futures::future::{err, ok};

    use core_context::Context;
    use core_network_message::consensus::{Proposal, Vote};
    use core_network_message::{Codec, Method};
    use core_runtime::ConsensusError;

    use crate::Error;

    use super::{ConsensusReactor, FutConsResult, InboundConsensus};

    struct MockConsensus {
        count:     AtomicUsize,
        reply_err: Arc<AtomicBool>,
    }

    impl MockConsensus {
        pub fn new() -> Self {
            MockConsensus {
                count:     AtomicUsize::new(0),
                reply_err: Arc::new(AtomicBool::new(false)),
            }
        }

        pub fn reply_err(&self, switch: bool) {
            self.reply_err.store(switch, Ordering::Relaxed);
        }

        pub fn count(&self) -> usize {
            self.count.load(Ordering::Relaxed)
        }

        fn error() -> Error {
            Error::ConsensusError(ConsensusError::Internal("mock error".to_owned()))
        }
    }

    impl InboundConsensus for MockConsensus {
        fn set_proposal(&self, _: Context, _: Vec<u8>) -> FutConsResult {
            if self.reply_err.load(Ordering::Relaxed) {
                Box::pin(err(Self::error()))
            } else {
                self.count.fetch_add(1, Ordering::Relaxed);
                Box::pin(ok(()))
            }
        }

        fn set_vote(&self, _: Context, _: Vec<u8>) -> FutConsResult {
            if self.reply_err.load(Ordering::Relaxed) {
                Box::pin(err(Self::error()))
            } else {
                self.count.fetch_add(1, Ordering::Relaxed);
                Box::pin(ok(()))
            }
        }
    }

    fn new_cons_reactor() -> ConsensusReactor<MockConsensus> {
        let cons = Arc::new(MockConsensus::new());

        ConsensusReactor::new(cons)
    }

    #[test]
    fn test_react_with_unknown_method() {
        let reactor = new_cons_reactor();
        let ctx = Context::new();
        let method = Method::SyncPullTxs;
        let data = b"software from".to_vec();

        match block_on(reactor.react(ctx, method, data)) {
            Err(Error::UnknownMethod(m)) => assert_eq!(m, method.to_u32()),
            _ => panic!("should return Error::UnknownMethod"),
        }
    }

    #[test]
    fn test_react_proposal() {
        let reactor = new_cons_reactor();

        let proposal = Proposal::from(b"fish man?".to_vec());
        let data = <Proposal as Codec>::encode(&proposal).unwrap().to_vec();
        let ctx = Context::new();
        let method = Method::Proposal;

        let maybe_ok = block_on(reactor.react(ctx, method, data));

        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.consensus.count(), 1);
    }

    #[test]
    fn test_react_proposal_with_bad_data() {
        let reactor = new_cons_reactor();
        let ctx = Context::new();
        let method = Method::Proposal;

        match block_on(reactor.react(ctx, method, vec![1, 2, 3])) {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[test]
    fn test_react_proposal_with_consensus_faiure() {
        let reactor = new_cons_reactor();
        let ctx = Context::new();
        let method = Method::Proposal;

        let proposal = Proposal::from(b"fish man?".to_vec());
        let data = <Proposal as Codec>::encode(&proposal).unwrap().to_vec();

        reactor.consensus.reply_err(true);
        match block_on(reactor.react(ctx, method, data)) {
            Err(Error::ConsensusError(ConsensusError::Internal(str))) => {
                assert!(str.contains("mock error"))
            }
            _ => panic!("should return Error::ConsensusError"),
        }
    }

    #[test]
    fn test_react_vote() {
        let reactor = new_cons_reactor();

        let vote = Vote::from(b"7ff ch 2".to_vec());
        let data = <Vote as Codec>::encode(&vote).unwrap().to_vec();
        let ctx = Context::new();
        let method = Method::Vote;

        let maybe_ok = block_on(reactor.react(ctx, method, data));

        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.consensus.count(), 1);
    }

    #[test]
    fn test_react_vote_with_bad_data() {
        let reactor = new_cons_reactor();
        let ctx = Context::new();
        let method = Method::Vote;

        match block_on(reactor.react(ctx, method, vec![1, 2, 3])) {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[test]
    fn test_react_vote_with_consensus_faiure() {
        let reactor = new_cons_reactor();
        let ctx = Context::new();
        let method = Method::Vote;

        let vote = Vote::from(b"eve".to_vec());
        let data = <Vote as Codec>::encode(&vote).unwrap().to_vec();

        reactor.consensus.reply_err(true);
        match block_on(reactor.react(ctx, method, data)) {
            Err(Error::ConsensusError(ConsensusError::Internal(str))) => {
                assert!(str.contains("mock error"))
            }
            _ => panic!("should return Error::ConsensusError"),
        }
    }
}
