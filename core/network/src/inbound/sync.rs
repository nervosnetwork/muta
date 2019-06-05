use std::clone::Clone;
use std::sync::Arc;

use futures::future::{BoxFuture, TryFutureExt};
use log::error;

use core_context::Context;
use core_network_message::common::{PullTxs, PushTxs};
use core_network_message::sync::{BroadcastStatus, PullBlocks, PushBlocks};
use core_network_message::{Codec, Method};
use core_runtime::Synchronization;
use core_types::{Block, Hash, SignedTransaction};

use crate::callback_map::Callback;
use crate::common::{scope_from_context, session_id_from_context};
use crate::inbound::{FutReactResult, Reactor};
use crate::outbound::DataEncoder;
use crate::p2p::Scope;
use crate::{DefaultOutboundHandle, Error};

pub type FutResult<T> = BoxFuture<'static, Result<T, Error>>;

pub trait InboundSynchronization: Send + Sync {
    fn sync_blocks(&self, ctx: Context, global_height: u64) -> FutResult<()>;

    fn get_blocks(&self, ctx: Context, heights: Vec<u64>) -> FutResult<Vec<Block>>;

    fn get_stxs(&self, ctx: Context, hashes: Vec<Hash>) -> FutResult<Vec<SignedTransaction>>;
}

pub trait SessionOutbound: Send + Sync + Clone {
    fn session_broadcast<D: DataEncoder>(&self, m: Method, d: D, s: Scope) -> Result<(), Error>;
}

pub struct SyncReactor<S, O> {
    sync: Arc<S>,

    outbound: O,
    callback: Arc<Callback>,
}

impl<S, O> Clone for SyncReactor<S, O>
where
    O: Clone,
{
    fn clone(&self) -> Self {
        SyncReactor {
            sync: Arc::clone(&self.sync),

            outbound: self.outbound.clone(),
            callback: Arc::clone(&self.callback),
        }
    }
}

impl<S, O> SyncReactor<S, O>
where
    S: InboundSynchronization + 'static,
    O: SessionOutbound + 'static,
{
    pub fn new(sync: Arc<S>, callback: Arc<Callback>, outbound: O) -> Self {
        SyncReactor {
            sync,

            outbound,
            callback,
        }
    }

    pub async fn react(&self, ctx: Context, method: Method, data: Vec<u8>) -> Result<(), Error> {
        match method {
            Method::SyncBroadcastStatus => self.handle_broadcast_status(ctx, data).await?,
            Method::SyncPullBlocks => self.handle_pull_blocks(ctx, data).await?,
            Method::SyncPushBlocks => self.handle_push_blocks(ctx, data).await?,
            Method::SyncPullTxs => self.handle_pull_txs(ctx, data).await?,
            Method::SyncPushTxs => self.handle_push_txs(ctx, data).await?,
            _ => Err(Error::UnknownMethod(method.to_u32()))?,
        };

        Ok(())
    }

    pub async fn handle_broadcast_status(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let status = <BroadcastStatus as Codec>::decode(data.as_slice())?;

        self.sync.sync_blocks(ctx, status.height).await?;

        Ok(())
    }

    pub async fn handle_pull_blocks(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let scope = scope_from_context(&ctx)?;
        let outbound = self.outbound.clone();

        let PullBlocks { uid, heights } = <PullBlocks as Codec>::decode(data.as_slice())?;

        let blocks = self.sync.get_blocks(ctx.clone(), heights).await?;
        let push_blocks = PushBlocks::from(uid, blocks);

        if let Err(err) = outbound.session_broadcast(Method::SyncPushBlocks, push_blocks, scope) {
            error!("net [inbound]: push_blocks: [err: {:?}]", err);
        }

        Ok(())
    }

    pub async fn handle_push_blocks(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let callback = Arc::clone(&self.callback);

        let session_id = session_id_from_context(&ctx)?.value();
        let push_blocks = <PushBlocks as Codec>::decode(data.as_slice())?;
        let uid = push_blocks.uid;

        let done_tx = callback.take::<Vec<Block>>(uid, session_id)?;
        let blocks = push_blocks.des()?;

        done_tx.try_send(blocks)?;

        Ok(())
    }

    pub async fn handle_pull_txs(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let scope = scope_from_context(&ctx)?;
        let outbound = self.outbound.clone();

        let pull_txs = <PullTxs as Codec>::decode(data.as_slice())?;
        let uid = pull_txs.uid;
        let hashes = pull_txs.des()?;

        let stxs = self.sync.get_stxs(ctx.clone(), hashes).await?;
        let push_txs = PushTxs::from(uid, stxs);

        if let Err(err) = outbound.session_broadcast(Method::SyncPushTxs, push_txs, scope) {
            error!("net [inbound]: push_txs: [err: {:?}]", err);
        }

        Ok(())
    }

    pub async fn handle_push_txs(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let callback = Arc::clone(&self.callback);

        let session_id = session_id_from_context(&ctx)?.value();
        let push_txs = <PushTxs as Codec>::decode(data.as_slice())?;
        let uid = push_txs.uid;

        let done_tx = callback.take::<Vec<SignedTransaction>>(uid, session_id)?;
        let stxs = push_txs.des()?;

        done_tx.try_send(stxs)?;

        Ok(())
    }
}

impl<S> InboundSynchronization for S
where
    S: Synchronization + 'static,
{
    fn sync_blocks(&self, ctx: Context, global_height: u64) -> FutResult<()> {
        Box::pin(self.sync_blocks(ctx, global_height).err_into())
    }

    fn get_blocks(&self, ctx: Context, heights: Vec<u64>) -> FutResult<Vec<Block>> {
        Box::pin(self.get_blocks(ctx, heights).err_into())
    }

    fn get_stxs(&self, ctx: Context, hashes: Vec<Hash>) -> FutResult<Vec<SignedTransaction>> {
        Box::pin(self.get_stxs(ctx, hashes).err_into())
    }
}

impl SessionOutbound for DefaultOutboundHandle {
    fn session_broadcast<D>(&self, method: Method, data: D, scope: Scope) -> Result<(), Error>
    where
        D: DataEncoder,
    {
        self.quick_broadcast(method, data, scope)
    }
}

impl<S, O> Reactor for SyncReactor<S, O>
where
    S: InboundSynchronization + 'static,
    O: SessionOutbound + 'static,
{
    fn react(&self, ctx: Context, method: Method, data: Vec<u8>) -> FutReactResult {
        let reactor = self.clone();

        Box::pin(async move { reactor.react(ctx, method, data).await })
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self as io, ErrorKind};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use core_context::{Context, P2P_SESSION_ID};
    use core_network_message::common::{PullTxs, PushTxs};
    use core_network_message::sync::{BroadcastStatus, PullBlocks, PushBlocks};
    use core_network_message::{Codec, Method};
    use core_runtime::SynchronizerError;
    use core_serialization::{Block as SerBlock, SignedTransaction as SerSignedTransaction};
    use core_types::{Block, Hash, SignedTransaction};

    use crate::callback_map::Callback;
    use crate::outbound::DataEncoder;
    use crate::p2p::{Bytes, Scope, SessionId};
    use crate::Error;

    use super::{FutResult, SyncReactor};
    use super::{InboundSynchronization, SessionOutbound};

    struct MockSynchronizer {
        reply_err: AtomicBool,
    }

    impl MockSynchronizer {
        pub fn new() -> Self {
            MockSynchronizer {
                reply_err: AtomicBool::new(false),
            }
        }

        pub fn reply_err(&self, switch: bool) {
            self.reply_err.store(switch, Ordering::Relaxed);
        }

        pub fn error() -> Error {
            Error::SynchronizerError(SynchronizerError::Internal("mock error".to_owned()))
        }
    }

    impl InboundSynchronization for MockSynchronizer {
        fn sync_blocks(&self, _: Context, _: u64) -> FutResult<()> {
            if self.reply_err.load(Ordering::Relaxed) {
                Box::pin(async move { Err(MockSynchronizer::error()) })
            } else {
                Box::pin(async move { Ok(()) })
            }
        }

        fn get_blocks(&self, _: Context, _: Vec<u64>) -> FutResult<Vec<Block>> {
            if self.reply_err.load(Ordering::Relaxed) {
                Box::pin(async move { Err(MockSynchronizer::error()) })
            } else {
                Box::pin(async move { Ok(vec![Block::default(), Block::default()]) })
            }
        }

        fn get_stxs(&self, _: Context, _: Vec<Hash>) -> FutResult<Vec<SignedTransaction>> {
            if self.reply_err.load(Ordering::Relaxed) {
                Box::pin(async move { Err(MockSynchronizer::error()) })
            } else {
                Box::pin(async move { Ok(vec![SignedTransaction::default()]) })
            }
        }
    }

    #[derive(Clone)]
    struct MockOutbound {
        reply_err:        Arc<AtomicBool>,
        broadcasted_data: Arc<Mutex<Option<(Bytes, Scope)>>>,
    }

    impl MockOutbound {
        pub fn new() -> Self {
            MockOutbound {
                reply_err:        Arc::new(AtomicBool::new(false)),
                broadcasted_data: Arc::new(Mutex::new(None)),
            }
        }

        pub fn reply_err(&self, switch: bool) {
            self.reply_err.store(switch, Ordering::Relaxed);
        }

        pub fn broadcasted_data(&self) -> Option<(Bytes, Scope)> {
            self.broadcasted_data.lock().unwrap().take()
        }
    }

    impl SessionOutbound for MockOutbound {
        fn session_broadcast<D>(&self, m: Method, d: D, s: Scope) -> Result<(), Error>
        where
            D: DataEncoder,
        {
            if !self.reply_err.load(Ordering::Relaxed) {
                let bytes = d.encode(m)?;
                *self.broadcasted_data.lock().unwrap() = Some((bytes, s));

                Ok(())
            } else {
                let io_err = io::Error::new(ErrorKind::NotConnected, "broken");
                Err(Error::IoError(io_err))
            }
        }
    }

    fn new_sync_reactor() -> SyncReactor<MockSynchronizer, MockOutbound> {
        let synchronizer = Arc::new(MockSynchronizer::new());
        let outbound = MockOutbound::new();
        let callback = Arc::new(Callback::new());

        SyncReactor::new(synchronizer, callback, outbound)
    }

    #[runtime::test]
    async fn test_react_with_unknown_method() {
        let reactor = new_sync_reactor();
        let ctx = Context::new();
        let method = Method::Vote;

        match reactor.react(ctx, method, vec![1, 2, 3]).await {
            Err(Error::UnknownMethod(m)) => assert_eq!(m, method.to_u32()),
            _ => panic!("should return Error::UnknownMethod"),
        }
    }

    #[runtime::test]
    async fn test_react_broadcast_status() {
        let reactor = new_sync_reactor();
        let status = BroadcastStatus::from(Hash::default(), 20);
        let data = <BroadcastStatus as Codec>::encode(&status).unwrap();

        let ctx = Context::new();
        let method = Method::SyncBroadcastStatus;
        let maybe_ok = reactor.react(ctx, method, data.to_vec()).await;

        assert_eq!(maybe_ok.unwrap(), ())
    }

    #[runtime::test]
    async fn test_react_broadcast_status_with_bad_data() {
        let reactor = new_sync_reactor();
        let ctx = Context::new();
        let method = Method::SyncBroadcastStatus;

        match reactor.react(ctx, method, vec![1, 2, 3]).await {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_broadcast_status_with_sync_failure() {
        let reactor = new_sync_reactor();
        let status = BroadcastStatus::from(Hash::default(), 20);
        let data = <BroadcastStatus as Codec>::encode(&status).unwrap();

        let ctx = Context::new();
        let method = Method::SyncBroadcastStatus;

        reactor.sync.reply_err(true);
        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::SynchronizerError(SynchronizerError::Internal(str))) => {
                assert!(str.contains("mock error"))
            }
            _ => panic!("should return Error::SynchronizerError"),
        }
    }

    #[runtime::test]
    async fn test_react_pull_blocks() {
        let reactor = new_sync_reactor();
        let pull_blocks = PullBlocks::from(1, vec![1, 2]);
        let data = <PullBlocks as Codec>::encode(&pull_blocks).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPullBlocks;

        let push_blocks = PushBlocks::from(1, vec![Block::default(), Block::default()]);
        let bytes =
            <PushBlocks as DataEncoder>::encode(&push_blocks, Method::SyncPushBlocks).unwrap();
        let scope = Scope::Single(SessionId::new(1));

        let maybe_ok = reactor.react(ctx, method, data.to_vec()).await;

        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.outbound.broadcasted_data(), Some((bytes, scope)));
    }

    #[runtime::test]
    async fn test_react_pull_blocks_without_session_id() {
        let reactor = new_sync_reactor();
        let pull_blocks = PullBlocks::from(1, vec![1, 2]);
        let data = <PullBlocks as Codec>::encode(&pull_blocks).unwrap();

        let ctx = Context::new();
        let method = Method::SyncPullBlocks;

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::SessionIdNotFound) => (),
            _ => panic!("should return Error::SessionIdNotFound"),
        }
    }

    #[runtime::test]
    async fn test_react_pull_blocks_with_bad_data() {
        let reactor = new_sync_reactor();
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPullBlocks;

        match reactor.react(ctx, method, vec![1, 2, 3]).await {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_pull_blocks_with_sync_failure() {
        let reactor = new_sync_reactor();
        let pull_blocks = PullBlocks::from(1, vec![1, 2]);
        let data = <PullBlocks as Codec>::encode(&pull_blocks).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPullBlocks;
        reactor.sync.reply_err(true);

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::SynchronizerError(SynchronizerError::Internal(str))) => {
                assert!(str.contains("mock error"))
            }
            _ => panic!("should return Error::SynchronizerError"),
        }
    }

    #[runtime::test]
    async fn test_react_pull_blocks_with_broadcast_failure() {
        let reactor = new_sync_reactor();
        let pull_blocks = PullBlocks::from(1, vec![1, 2]);
        let data = <PullBlocks as Codec>::encode(&pull_blocks).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPullBlocks;
        reactor.outbound.reply_err(true);

        let maybe_ok = reactor.react(ctx, method, data.to_vec()).await;
        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.outbound.broadcasted_data(), None);
    }

    #[runtime::test]
    async fn test_react_push_blocks() {
        let reactor = new_sync_reactor();
        let push_blocks = PushBlocks::from(1, vec![Block::default()]);
        let data = <PushBlocks as Codec>::encode(&push_blocks).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushBlocks;
        let rx = reactor.callback.insert::<Vec<Block>>(1, 1);

        let maybe_ok = reactor.react(ctx, method, data.to_vec()).await;
        assert_eq!(maybe_ok.unwrap(), ());

        let blocks = rx.try_recv().unwrap();
        assert_eq!(blocks.len(), 1);
    }

    #[runtime::test]
    async fn test_react_push_blocks_without_session_id() {
        let reactor = new_sync_reactor();
        let push_blocks = PushBlocks::from(1, vec![Block::default()]);
        let data = <PushBlocks as Codec>::encode(&push_blocks).unwrap();

        let ctx = Context::new();
        let method = Method::SyncPushBlocks;

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::SessionIdNotFound) => (),
            _ => panic!("should return Error::SessionIdNotFound"),
        }
    }

    #[runtime::test]
    async fn test_react_push_blocks_with_bad_data() {
        let reactor = new_sync_reactor();
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushBlocks;

        match reactor.react(ctx, method, vec![1, 2, 3]).await {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_push_blocks_without_cb_tx() {
        let reactor = new_sync_reactor();
        let push_blocks = PushBlocks::from(1, vec![Block::default()]);
        let data = <PushBlocks as Codec>::encode(&push_blocks).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushBlocks;

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::CallbackItemNotFound(id)) => assert_eq!(id, 1),
            _ => panic!("should return Error::CallbackItemNotFound"),
        }
    }

    #[runtime::test]
    async fn test_react_push_blocks_with_wrong_cb_tx() {
        let reactor = new_sync_reactor();
        let push_blocks = PushBlocks::from(1, vec![Block::default()]);
        let data = <PushBlocks as Codec>::encode(&push_blocks).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushBlocks;
        let _rx = reactor.callback.insert::<Vec<String>>(1, 1);

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::CallbackItemWrongType(id)) => assert_eq!(id, 1),
            _ => panic!("should return Error::CallbackItemWrongType"),
        }
    }

    #[runtime::test]
    async fn test_react_push_blocks_with_bad_ser_block() {
        let reactor = new_sync_reactor();
        let mut ser_block = SerBlock::default();
        ser_block.header = None;

        let push_blocks = PushBlocks {
            uid:    1,
            blocks: vec![ser_block],
        };
        let data = <PushBlocks as Codec>::encode(&push_blocks).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushBlocks;
        let _rx = reactor.callback.insert::<Vec<Block>>(1, 1);

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::SerCodecError(_)) => (),
            _ => panic!("should return Error::SerCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_push_blocks_with_tx_failure() {
        let reactor = new_sync_reactor();
        let push_blocks = PushBlocks::from(1, vec![Block::default()]);
        let data = <PushBlocks as Codec>::encode(&push_blocks).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushBlocks;
        let rx = reactor.callback.insert::<Vec<Block>>(1, 1);
        drop(rx);

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::ChannelTrySendError(_)) => (),
            _ => panic!("should return Error::ChannelTrySendError"),
        }
    }

    #[runtime::test]
    async fn test_react_pull_txs() {
        let reactor = new_sync_reactor();
        let pull_txs = PullTxs::from(1, vec![Hash::default()]);
        let data = <PullTxs as Codec>::encode(&pull_txs).unwrap();

        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let bytes = <PushTxs as DataEncoder>::encode(&push_txs, Method::SyncPushTxs).unwrap();
        let scope = Scope::Single(SessionId::new(1));

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPullTxs;

        let maybe_ok = reactor.react(ctx, method, data.to_vec()).await;

        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.outbound.broadcasted_data(), Some((bytes, scope)));
    }

    #[runtime::test]
    async fn test_pull_txs_without_session_id() {
        let reactor = new_sync_reactor();
        let ctx = Context::new();

        match reactor.react(ctx, Method::SyncPullTxs, vec![1, 2, 3]).await {
            Err(Error::SessionIdNotFound) => (),
            _ => panic!("should return Error::SessionIdNotFound"),
        }
    }

    #[runtime::test]
    async fn test_pull_txs_with_bad_data() {
        let reactor = new_sync_reactor();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);

        match reactor.react(ctx, Method::SyncPullTxs, vec![1, 2, 3]).await {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[runtime::test]
    async fn test_pull_txs_with_bad_ser_hash() {
        let reactor = new_sync_reactor();
        let pull_txs = PullTxs {
            uid:    1,
            hashes: vec![vec![1, 2, 3]],
        };
        let data = <PullTxs as Codec>::encode(&pull_txs).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPullTxs;

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::SerCodecError(_)) => (),
            _ => panic!("should return Error::SerCodecError"),
        }
    }

    #[runtime::test]
    async fn test_pull_txs_with_sync_get_txs_failure() {
        let reactor = new_sync_reactor();
        let pull_txs = PullTxs::from(1, vec![Hash::default()]);
        let data = <PullTxs as Codec>::encode(&pull_txs).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPullTxs;

        reactor.sync.reply_err(true);
        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::SynchronizerError(SynchronizerError::Internal(str))) => {
                assert!(str.contains("mock error"))
            }
            _ => panic!("should return Error::SynchronizerError"),
        }
    }

    #[runtime::test]
    async fn test_pull_txs_with_outbound_failure() {
        let reactor = new_sync_reactor();
        let pull_txs = PullTxs::from(1, vec![Hash::default()]);
        let data = <PullTxs as Codec>::encode(&pull_txs).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPullTxs;

        reactor.outbound.reply_err(true);
        let maybe_ok = reactor.react(ctx, method, data.to_vec()).await;

        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.outbound.broadcasted_data(), None);
    }

    #[runtime::test]
    async fn test_react_push_txs() {
        let reactor = new_sync_reactor();
        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 2);
        let method = Method::SyncPushTxs;
        let rx = reactor.callback.insert::<Vec<SignedTransaction>>(1, 2);

        let maybe_ok = reactor.react(ctx, method, data.to_vec()).await;
        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(rx.try_recv().unwrap(), vec![SignedTransaction::default()]);
    }

    #[runtime::test]
    async fn test_react_push_txs_without_session_id() {
        let reactor = new_sync_reactor();
        let ctx = Context::new();

        match reactor.react(ctx, Method::SyncPushTxs, vec![1, 2]).await {
            Err(Error::SessionIdNotFound) => (),
            _ => panic!("should return Error::SessionIdNotFound"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_with_bad_data() {
        let reactor = new_sync_reactor();
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);

        match reactor.react(ctx, Method::SyncPushTxs, vec![1, 2]).await {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_without_cb_tx() {
        let reactor = new_sync_reactor();
        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushTxs;

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::CallbackItemNotFound(id)) => assert_eq!(id, 1),
            _ => panic!("should return Error::CallbackItemNotFound"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_with_wrong_cb_type() {
        let reactor = new_sync_reactor();
        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushTxs;
        let _rx = reactor.callback.insert::<Vec<String>>(1, 1);

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::CallbackItemWrongType(id)) => assert_eq!(id, 1),
            _ => panic!("should return Error::CallbackItemWrongType"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_with_bad_ser_stxs() {
        let reactor = new_sync_reactor();
        let mut ser_stx = SerSignedTransaction::default();
        ser_stx.untx = None;

        let push_txs = PushTxs {
            uid:     1,
            sig_txs: vec![ser_stx],
        };
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushTxs;
        let _rx = reactor.callback.insert::<Vec<SignedTransaction>>(1, 1);

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::SerCodecError(_)) => (),
            _ => panic!("should return Error::SerCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_with_cb_tx_failure() {
        let reactor = new_sync_reactor();
        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::SyncPushTxs;
        let rx = reactor.callback.insert::<Vec<SignedTransaction>>(1, 1);
        drop(rx);

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::ChannelTrySendError(_)) => (),
            _ => panic!("should return Error::ChannelTrySendError"),
        }
    }
}
