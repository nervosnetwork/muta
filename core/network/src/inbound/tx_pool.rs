use std::clone::Clone;
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::prelude::{StreamExt, TryFutureExt};
use futures::stream;

use core_context::Context;
use core_network_message::common::{PullTxs, PushTxs};
use core_network_message::tx_pool::BroadcastTxs;
use core_network_message::{Codec, Method};
use core_runtime::TransactionPool;
use core_types::{Hash, SignedTransaction, UnverifiedTransaction};

use crate::callback_map::Callback;
use crate::common::{scope_from_context, session_id_from_context};
use crate::inbound::{FutReactResult, Reactor};
use crate::outbound::DataEncoder;
use crate::p2p::Scope;
use crate::{DefaultOutboundHandle, Error};

pub type FutTxPoolResult<T> = BoxFuture<'static, Result<T, Error>>;

pub trait SessionBroadcaster: Send + Sync + Clone {
    fn session_broadcast<D: DataEncoder>(&self, m: Method, data: D, s: Scope) -> Result<(), Error>;
}

pub trait InboundTransactionPool: Send + Sync {
    fn insert(&self, c: Context, u: UnverifiedTransaction) -> FutTxPoolResult<SignedTransaction>;

    fn get_batch(&self, ctx: Context, hashes: &[Hash]) -> FutTxPoolResult<Vec<SignedTransaction>>;
}

pub struct TxPoolReactor<B, P> {
    outbound: B,
    callback: Arc<Callback>,

    tx_pool: Arc<P>,
}

impl<B, P> Clone for TxPoolReactor<B, P>
where
    B: Clone,
{
    fn clone(&self) -> Self {
        TxPoolReactor {
            outbound: self.outbound.clone(),
            callback: Arc::clone(&self.callback),

            tx_pool: Arc::clone(&self.tx_pool),
        }
    }
}

impl<B, P> TxPoolReactor<B, P>
where
    B: SessionBroadcaster,
    P: InboundTransactionPool + 'static,
{
    pub fn new(outbound: B, callback: Arc<Callback>, tx_pool: Arc<P>) -> Self {
        TxPoolReactor {
            outbound,
            callback,

            tx_pool,
        }
    }

    pub async fn react(&self, ctx: Context, method: Method, data: Vec<u8>) -> Result<(), Error> {
        match method {
            Method::BroadcastTxs => self.handle_broadcast_txs(ctx, data).await?,
            Method::PullTxs => self.handle_pull_txs(ctx, data).await?,
            Method::PushTxs => self.handle_push_txs(ctx, data).await?,
            _ => Err(Error::UnknownMethod(method.to_u32()))?,
        };

        Ok(())
    }

    pub async fn handle_broadcast_txs(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let broadcast_txs = <BroadcastTxs as Codec>::decode(data.as_slice())?;
        let mut sig_txs = stream::iter(broadcast_txs.des()?);

        while let Some(stx) = sig_txs.next().await {
            let tx_pool = Arc::clone(&self.tx_pool);

            // FIXME: spawn insertion, but will cause too many spawned jobs
            // TODO: Error with context support, so we can handle error upward
            if let Err(err) = tx_pool.insert(ctx.clone(), stx.untx).await {
                let hash = stx.hash.as_hex();

                log::warn!("net [in]: tx_pool: [hash: {}, err: {:?}]", hash, err);
            }
        }

        Ok(())
    }

    pub async fn handle_pull_txs(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let scope = scope_from_context(&ctx)?;

        let tx_pool = Arc::clone(&self.tx_pool);
        let outbound = self.outbound.clone();

        let pull_txs = <PullTxs as Codec>::decode(data.as_slice())?;
        let uid = pull_txs.uid;
        let hashes = pull_txs.des()?;

        let txs = tx_pool.get_batch(ctx.clone(), hashes.as_slice()).await?;
        let push_txs = PushTxs::from(uid, txs);

        if let Err(err) = outbound.session_broadcast(Method::PushTxs, push_txs, scope) {
            log::warn!("net [in]: pull_txs: [err: {:?}]", err);
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

impl SessionBroadcaster for DefaultOutboundHandle {
    fn session_broadcast<D: DataEncoder>(&self, m: Method, data: D, s: Scope) -> Result<(), Error> {
        self.quick_broadcast(m, data, s)
    }
}

impl<P> InboundTransactionPool for P
where
    P: TransactionPool,
{
    fn insert(&self, c: Context, u: UnverifiedTransaction) -> FutTxPoolResult<SignedTransaction> {
        Box::pin(self.insert(c, u).err_into())
    }

    fn get_batch(&self, ctx: Context, hashes: &[Hash]) -> FutTxPoolResult<Vec<SignedTransaction>> {
        Box::pin(self.get_batch(ctx, hashes).err_into())
    }
}

impl<B, P> Reactor for TxPoolReactor<B, P>
where
    B: SessionBroadcaster + 'static,
    P: InboundTransactionPool + 'static,
{
    fn react(&self, ctx: Context, method: Method, data: Vec<u8>) -> FutReactResult {
        let reactor = self.clone();

        Box::pin(async move { reactor.react(ctx, method, data).await })
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self as io, ErrorKind};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use futures::future::{err, ok};

    use core_context::{Context, P2P_SESSION_ID};
    use core_network_message::common::{PullTxs, PushTxs};
    use core_network_message::tx_pool::BroadcastTxs;
    use core_network_message::{Codec, Method};
    use core_runtime::TransactionPoolError;
    use core_serialization::SignedTransaction as SerSignedTransaction;
    use core_types::{Hash, SignedTransaction, UnverifiedTransaction};

    use crate::callback_map::Callback;
    use crate::outbound::DataEncoder;
    use crate::p2p::{Bytes, Scope, SessionId};
    use crate::Error;

    use super::FutTxPoolResult;
    use super::{InboundTransactionPool, SessionBroadcaster, TxPoolReactor};

    #[derive(Clone)]
    struct MockBroadcaster {
        reply_err:        Arc<AtomicBool>,
        broadcasted_data: Arc<Mutex<Option<(Bytes, Scope)>>>,
    }

    impl MockBroadcaster {
        pub fn new() -> Self {
            MockBroadcaster {
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

    impl SessionBroadcaster for MockBroadcaster {
        fn session_broadcast<D>(&self, m: Method, data: D, s: Scope) -> Result<(), Error>
        where
            D: DataEncoder,
        {
            if !self.reply_err.load(Ordering::Relaxed) {
                let bytes = data.encode(m)?;
                *self.broadcasted_data.lock().unwrap() = Some((bytes, s));

                Ok(())
            } else {
                let io_err = io::Error::new(ErrorKind::NotConnected, "broken");
                Err(Error::IoError(io_err))
            }
        }
    }

    struct MockTransactionPool {
        count:     Arc<AtomicUsize>,
        reply_err: Arc<AtomicBool>,
    }

    impl MockTransactionPool {
        pub fn new() -> Self {
            MockTransactionPool {
                count:     Arc::new(AtomicUsize::new(0)),
                reply_err: Arc::new(AtomicBool::new(false)),
            }
        }

        pub fn count(&self) -> usize {
            self.count.load(Ordering::Relaxed)
        }

        pub fn reply_err(&self, switch: bool) {
            self.reply_err.store(switch, Ordering::Relaxed);
        }
    }

    impl InboundTransactionPool for MockTransactionPool {
        fn insert(
            &self,
            _: Context,
            _: UnverifiedTransaction,
        ) -> FutTxPoolResult<SignedTransaction> {
            if self.reply_err.load(Ordering::Relaxed) {
                Box::pin(err(Error::TransactionPoolError(
                    TransactionPoolError::NotExpected,
                )))
            } else {
                self.count.fetch_add(1, Ordering::Relaxed);

                Box::pin(ok(SignedTransaction::default()))
            }
        }

        fn get_batch(
            &self,
            _: Context,
            hashes: &[Hash],
        ) -> FutTxPoolResult<Vec<SignedTransaction>> {
            if self.reply_err.load(Ordering::Relaxed) {
                Box::pin(err(Error::TransactionPoolError(
                    TransactionPoolError::TransactionNotFound,
                )))
            } else {
                self.count.fetch_add(hashes.len(), Ordering::Relaxed);

                Box::pin(ok(vec![
                    SignedTransaction::default(),
                    SignedTransaction::default(),
                ]))
            }
        }
    }

    fn new_tx_pool_reactor() -> (
        TxPoolReactor<MockBroadcaster, MockTransactionPool>,
        Arc<Callback>,
    ) {
        let cb = Arc::new(Callback::new());
        let broadcaster = MockBroadcaster::new();
        let tx_pool = Arc::new(MockTransactionPool::new());

        let reactor = TxPoolReactor::new(broadcaster, Arc::clone(&cb), tx_pool);
        (reactor, cb)
    }

    #[runtime::test]
    async fn test_react_with_unknown_method() {
        let (reactor, _) = new_tx_pool_reactor();

        let stxs = vec![SignedTransaction::default(), SignedTransaction::default()];
        let broadcast_txs = BroadcastTxs::from(stxs);
        let data = <BroadcastTxs as Codec>::encode(&broadcast_txs).unwrap();

        let ctx = Context::new();
        let method = Method::SyncPullTxs;

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::UnknownMethod(m)) => assert_eq!(m, method.to_u32()),
            _ => panic!("should return Error::UnknownMethod"),
        }
    }

    #[runtime::test]
    async fn test_react_broadcast_txs() {
        let (reactor, _) = new_tx_pool_reactor();
        let stxs = vec![SignedTransaction::default(), SignedTransaction::default()];
        let broadcast_txs = BroadcastTxs::from(stxs);
        let data = <BroadcastTxs as Codec>::encode(&broadcast_txs).unwrap();

        let ctx = Context::new();
        let method = Method::BroadcastTxs;
        let maybe_ok = reactor.react(ctx, method, data.to_vec()).await;

        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.tx_pool.count(), 2);
    }

    #[runtime::test]
    async fn test_react_broadcast_txs_with_bad_data() {
        let (reactor, _) = new_tx_pool_reactor();

        let ctx = Context::new();
        let method = Method::BroadcastTxs;

        match reactor.react(ctx, method, vec![1, 2, 3]).await {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_broadcast_txs_with_des_failure() {
        let (reactor, _) = new_tx_pool_reactor();
        let mut ser_stx = SerSignedTransaction::default();
        ser_stx.untx = None;

        let broadcast_txs = BroadcastTxs { txs: vec![ser_stx] };
        let data = <BroadcastTxs as Codec>::encode(&broadcast_txs).unwrap();

        let ctx = Context::new();
        let method = Method::BroadcastTxs;

        match reactor.react(ctx, method, data.to_vec()).await {
            Err(Error::SerCodecError(_)) => (),
            _ => panic!("should return Error::SerCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_broadcast_txs_with_insertion_failure() {
        let (reactor, _) = new_tx_pool_reactor();
        let broadcast_txs = BroadcastTxs::from(vec![SignedTransaction::default()]);
        let data = <BroadcastTxs as Codec>::encode(&broadcast_txs).unwrap();

        let ctx = Context::new();
        let method = Method::BroadcastTxs;
        reactor.tx_pool.reply_err(true);

        let maybe_ok = reactor.react(ctx, method, data.to_vec()).await;

        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.tx_pool.count(), 0);
    }

    #[runtime::test]
    async fn test_react_pull_txs() {
        let (reactor, _) = new_tx_pool_reactor();
        let pull_txs = PullTxs::from(1, vec![Hash::default(), Hash::default()]);
        let data = <PullTxs as Codec>::encode(&pull_txs).unwrap().to_vec();

        let stxs = {
            let maybe_stxs = reactor.tx_pool.get_batch(Context::new(), &[]).await;
            maybe_stxs.unwrap()
        };
        let push_txs = PushTxs::from(1, stxs);
        let bytes = <PushTxs as DataEncoder>::encode(&push_txs, Method::PushTxs).unwrap();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let scope = Scope::Single(SessionId::new(1));
        let method = Method::PullTxs;

        let maybe_ok = reactor.react(ctx, method, data).await;

        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.tx_pool.count(), 2);
        assert_eq!(reactor.outbound.broadcasted_data(), Some((bytes, scope)));
    }

    #[runtime::test]
    async fn test_react_pull_txs_without_session_id() {
        let (reactor, _) = new_tx_pool_reactor();
        let pull_txs = PullTxs::from(1, vec![Hash::default(), Hash::default()]);
        let data = <PullTxs as Codec>::encode(&pull_txs).unwrap().to_vec();

        let ctx = Context::new();
        let method = Method::PullTxs;

        match reactor.react(ctx, method, data).await {
            Err(Error::SessionIdNotFound) => (),
            _ => panic!("should return Error::SessionIdNotFound"),
        }
    }

    #[runtime::test]
    async fn test_react_pull_txs_with_bad_data() {
        let (reactor, _) = new_tx_pool_reactor();
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::PullTxs;

        match reactor.react(ctx, method, vec![1, 2, 3, 4]).await {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_pull_txs_with_bad_hash() {
        let (reactor, _) = new_tx_pool_reactor();
        let pull_txs = PullTxs {
            uid:    1,
            hashes: vec![vec![1, 2, 3]],
        };
        let data = <PullTxs as Codec>::encode(&pull_txs).unwrap().to_vec();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::PullTxs;

        match reactor.react(ctx, method, data).await {
            Err(Error::SerCodecError(_)) => (),
            _ => panic!("should return Error::SerCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_pull_txs_with_get_batch_failure() {
        let (reactor, _) = new_tx_pool_reactor();
        let pull_txs = PullTxs::from(1, vec![Hash::default(), Hash::default()]);
        let data = <PullTxs as Codec>::encode(&pull_txs).unwrap().to_vec();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::PullTxs;
        reactor.tx_pool.reply_err(true);

        match reactor.react(ctx, method, data).await {
            Err(Error::TransactionPoolError(TransactionPoolError::TransactionNotFound)) => (),
            _ => panic!("should return Error::TransactionPoolError"),
        }
    }

    #[runtime::test]
    async fn test_react_pull_txs_with_broadcast_failure() {
        let (reactor, _) = new_tx_pool_reactor();
        let pull_txs = PullTxs::from(1, vec![Hash::default(), Hash::default()]);
        let data = <PullTxs as Codec>::encode(&pull_txs).unwrap().to_vec();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::PullTxs;
        reactor.outbound.reply_err(true);

        let maybe_ok = reactor.react(ctx, method, data).await;
        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(reactor.outbound.broadcasted_data(), None);
    }

    #[runtime::test]
    async fn test_react_push_txs() {
        let (reactor, cb) = new_tx_pool_reactor();
        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap().to_vec();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let rx = cb.insert::<Vec<SignedTransaction>>(1, 1);
        let method = Method::PushTxs;

        let maybe_ok = reactor.react(ctx, method, data).await;

        assert_eq!(maybe_ok.unwrap(), ());
        assert_eq!(rx.try_recv().unwrap(), vec![SignedTransaction::default()]);
    }

    #[runtime::test]
    async fn test_react_push_txs_without_session_id() {
        let (reactor, _) = new_tx_pool_reactor();
        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap().to_vec();

        let ctx = Context::new();
        let method = Method::PushTxs;

        match reactor.react(ctx, method, data).await {
            Err(Error::SessionIdNotFound) => (),
            _ => panic!("should return Error::SessionIdNotFound"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_with_bad_data() {
        let (reactor, _) = new_tx_pool_reactor();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::PushTxs;

        match reactor.react(ctx, method, vec![1, 2, 3]).await {
            Err(Error::MsgCodecError(_)) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_with_cb_item_not_found() {
        let (reactor, _) = new_tx_pool_reactor();
        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap().to_vec();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::PushTxs;

        match reactor.react(ctx, method, data).await {
            Err(Error::CallbackItemNotFound(id)) => assert_eq!(id, 1),
            _ => panic!("should return Error::CallbackItemNotFound"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_with_cb_item_wrong_type() {
        let (reactor, cb) = new_tx_pool_reactor();
        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap().to_vec();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::PushTxs;
        let _rx = cb.insert::<Vec<String>>(1, 1);

        match reactor.react(ctx, method, data).await {
            Err(Error::CallbackItemWrongType(id)) => assert_eq!(id, 1),
            _ => panic!("should return Error::CallbackItemWrongType"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_with_des_failure() {
        let (reactor, cb) = new_tx_pool_reactor();
        let mut ser_stx = SerSignedTransaction::default();
        ser_stx.untx = None;

        let push_txs = PushTxs {
            uid:     1,
            sig_txs: vec![ser_stx],
        };
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap().to_vec();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::PushTxs;
        let _rx = cb.insert::<Vec<SignedTransaction>>(1, 1);

        match reactor.react(ctx, method, data).await {
            Err(Error::SerCodecError(_)) => (),
            _ => panic!("should return Error::SerCodecError"),
        }
    }

    #[runtime::test]
    async fn test_react_push_txs_with_try_send_failure() {
        let (reactor, cb) = new_tx_pool_reactor();
        let push_txs = PushTxs::from(1, vec![SignedTransaction::default()]);
        let data = <PushTxs as Codec>::encode(&push_txs).unwrap().to_vec();

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1);
        let method = Method::PushTxs;
        let rx = cb.insert::<Vec<SignedTransaction>>(1, 1);
        drop(rx);

        match reactor.react(ctx, method, data).await {
            Err(Error::ChannelTrySendError(_)) => (),
            _ => panic!("should return Error::ChannelTrySendError"),
        }
    }
}
