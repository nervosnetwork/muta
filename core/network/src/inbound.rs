pub mod consensus;
pub mod core_sync;
pub mod sync;
pub mod tx_pool;
pub use consensus::ConsensusReactor;
pub use sync::SyncReactor;
pub use tx_pool::TxPoolReactor;

use std::collections::HashMap;
use std::future::Future;
use std::io::{self as io, ErrorKind};
use std::pin::Pin;
use std::sync::Arc;

use futures::future::{ready, BoxFuture};
use futures::prelude::{FutureExt, Stream, TryFutureExt};
use futures::task::{Context as TaskContext, Poll};
use log::{error, info};

use common_channel::Receiver;
use core_context::Context;
use core_context::P2P_SESSION_ID;
use core_network_message::{Codec, Component, Message, Method};
use core_runtime::{Consensus, Storage, Synchronization, TransactionPool};

use crate::callback_map::Callback;
use crate::inbound::core_sync::Synchronizer;
use crate::p2p::{Bytes, SessionMessage};
use crate::{DefaultOutboundHandle as OutboundHandle, Error};

pub type FutReactResult = Pin<Box<dyn Future<Output = Result<(), Error>> + 'static + Send>>;
pub type BoxReactor = Box<dyn Reactor + Send + Sync>;
pub type Data = Vec<u8>;

pub trait Reactor {
    fn react(&self, ctx: Context, method: Method, data: Vec<u8>) -> FutReactResult;
}

pub trait DataDecoder {
    fn decode(raw: &Bytes) -> Result<(Method, Data), Error>;
}

pub struct Reactors(HashMap<Component, BoxReactor>);

impl Reactors {
    pub fn builder(cap: usize) -> Self {
        Reactors(HashMap::with_capacity(cap))
    }

    pub fn pool_reactor<P>(mut self, pool: Arc<P>, out: OutboundHandle, cb: Arc<Callback>) -> Self
    where
        P: TransactionPool + 'static,
    {
        let reactor: BoxReactor = Box::new(TxPoolReactor::new(out, cb, pool));
        self.0.insert(Component::TxPool, reactor);

        self
    }

    pub fn consensus_reactor<C>(mut self, cons: Arc<C>) -> Self
    where
        C: Consensus + 'static,
    {
        let reactor: BoxReactor = Box::new(ConsensusReactor::new(cons));
        self.0.insert(Component::Consensus, reactor);

        self
    }

    pub fn synchronizer<C, S>(c: Arc<C>, s: Arc<S>, out: OutboundHandle) -> Synchronizer<C, S>
    where
        C: Consensus + 'static,
        S: Storage + 'static,
    {
        Synchronizer::new(c, s, out)
    }

    pub fn sync_reactor<S>(mut self, sync: Arc<S>, out: OutboundHandle, cb: Arc<Callback>) -> Self
    where
        S: Synchronization + 'static,
    {
        let reactor: BoxReactor = Box::new(SyncReactor::new(sync, cb, out));
        self.0.insert(Component::Synch, reactor);

        self
    }

    pub fn build(self) -> Arc<Self> {
        Arc::new(self)
    }
}

pub struct InboundHandle {
    inbound:  Receiver<SessionMessage>,
    reactors: Arc<Reactors>,
}

impl InboundHandle {
    pub fn new(inbound: Receiver<SessionMessage>, reactors: Arc<Reactors>) -> Self {
        InboundHandle { inbound, reactors }
    }

    async fn handle_body(reactors: Arc<Reactors>, ctx: Context, body: Bytes) -> Result<(), Error> {
        let (method, data) = <Message as DataDecoder>::decode(&body)?;
        let reactor = reactors.0.get(&method.component()) // rustfmt :)
            .ok_or_else(|| Error::UnknownMethod(method.to_u32()))?;

        reactor.react(ctx, method, data).await?;
        Ok(())
    }

    // TODO: logger instead of log fn directly
    fn handle_msg(&self, sess_msg: SessionMessage) -> BoxFuture<'static, ()> {
        let SessionMessage { id, addr, body } = sess_msg;
        let ctx = core_context::Context::new().with_value(P2P_SESSION_ID, id.value());
        let reactors = Arc::clone(&self.reactors);

        let fut = Self::handle_body(reactors, ctx, body).then(move |maybe_ok| {
            // TODO: report error to peer manager
            if let Err(err) = maybe_ok {
                error!("net [in]: [addr: {:?}, err: {:?}]", addr, err);
            }
            ready(())
        });

        Box::pin(fut)
    }
}

impl Stream for InboundHandle {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        match Stream::poll_next(Pin::new(&mut self.inbound), ctx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                // TODO: check network state, shutdown or unexpected stop?
                // TODO: if shutdown, then do clean up
                // TODO: if unexpected stop, then try to restart
                info!("net [in]: stop");
                Poll::Ready(None)
            }
            Poll::Ready(Some(session_msg)) => {
                let job = self.handle_msg(session_msg);
                tokio::spawn(job.unit_error().boxed().compat());

                Poll::Ready(Some(()))
            }
        }
    }
}

impl DataDecoder for Message {
    fn decode(raw: &Bytes) -> Result<(Method, Data), Error> {
        let Message {
            method,
            data,
            data_size,
        } = <Message as Codec>::decode(raw)?;

        if data_size != data.len() as u64 {
            let corruption_err = io::Error::new(ErrorKind::UnexpectedEof, "data corruption");

            Err(Error::IoError(corruption_err))?
        }

        Ok((Method::from_u32(method)?, data.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::ErrorKind;
    use std::sync::Arc;

    use futures::executor::block_on;
    use futures::prelude::StreamExt;

    use common_channel::{bounded, Sender};
    use core_context::Context;
    use core_network_message::consensus::Proposal;
    use core_network_message::Error as NetMessageError;
    use core_network_message::{Codec, Component, Message, Method};

    use crate::outbound::DataEncoder;
    use crate::p2p::{Bytes, SessionId, SessionMessage};
    use crate::Error;

    use super::{BoxReactor, FutReactResult, Reactors};
    use super::{DataDecoder, InboundHandle, Reactor};

    struct MockReactor;

    impl Reactor for MockReactor {
        fn react(&self, _: Context, _: Method, _: Vec<u8>) -> FutReactResult {
            use futures::future::ok;

            Box::pin(ok(()))
        }
    }

    struct MockErrorReactor;

    impl Reactor for MockErrorReactor {
        fn react(&self, _: Context, _: Method, _: Vec<u8>) -> FutReactResult {
            use futures::future::err;

            Box::pin(err(Error::UnknownMethod(9_999_999)))
        }
    }

    fn encode_bytes<T: DataEncoder>(data: &T, method: Method) -> Bytes {
        <T as DataEncoder>::encode(&data, method).unwrap()
    }

    fn mock_wrong_size_data() -> Bytes {
        let proposal = Proposal::from(b"vote triss".to_vec());
        let data = <Proposal as Codec>::encode(&proposal).unwrap();

        let msg = Message {
            method:    Method::Proposal.to_u32(),
            data_size: data.len() as u64 + 1,
            data:      data.to_vec(),
        };

        <Message as Codec>::encode(&msg).unwrap()
    }

    fn mock_wrong_method_data() -> Bytes {
        let proposal = Proposal::from(b"vote triss".to_vec());
        let data = <Proposal as Codec>::encode(&proposal).unwrap();

        let msg = Message {
            method:    9_999_999, // wrong
            data_size: data.len() as u64,
            data:      data.to_vec(),
        };

        <Message as Codec>::encode(&msg).unwrap()
    }

    fn mock_sess_msg(body: Bytes) -> SessionMessage {
        let id = SessionId::new(1);
        let addr = "/ip4/127.0.0.1/tcp/2020".parse().unwrap();
        SessionMessage { id, addr, body }
    }

    fn mock_reactors() -> Arc<Reactors> {
        let reactor = Box::new(MockReactor) as BoxReactor;
        let err_reactor = Box::new(MockErrorReactor) as BoxReactor;

        let mut map = HashMap::with_capacity(2);
        map.insert(Component::TxPool, err_reactor);
        map.insert(Component::Consensus, reactor);

        Arc::new(Reactors(map))
    }

    fn mock_inbound() -> (InboundHandle, Sender<SessionMessage>) {
        let reactors = mock_reactors();
        let (tx, rx) = bounded(1);

        let inbound = InboundHandle {
            inbound: rx,
            reactors,
        };

        (inbound, tx)
    }

    #[test]
    fn test_data_decoder() {
        let proposal = Proposal::from(b"vote triss".to_vec());
        let expect_data = <Proposal as Codec>::encode(&proposal).unwrap();
        let bytes = encode_bytes(&proposal, Method::Proposal);

        let (method, data) = <Message as DataDecoder>::decode(&bytes).unwrap();
        assert_eq!(method, Method::Proposal);
        assert_eq!(data, expect_data);
    }

    #[test]
    fn test_data_decoder_with_wrong_msg_bytes() {
        let bad_data = b"12345".to_vec();
        match <Message as DataDecoder>::decode(&Bytes::from(bad_data)) {
            Err(Error::MsgCodecError(NetMessageError::DecodeError(_))) => (), // paas
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[test]
    fn test_data_decoder_with_wrong_data_size() {
        let bytes = mock_wrong_size_data();

        match <Message as DataDecoder>::decode(&bytes) {
            Err(Error::IoError(err)) => {
                assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
                assert!(format!("{:?}", err.get_ref()).contains("data corruption"));
            }
            _ => panic!("should return Error::IoError"),
        }
    }

    #[test]
    fn test_data_decoder_with_wrong_method() {
        let bytes = mock_wrong_method_data();

        match <Message as DataDecoder>::decode(&bytes) {
            Err(Error::UnknownMethod(m)) => assert_eq!(m, 9_999_999),
            _ => panic!("should return Error::UnknownMethod"),
        }
    }

    #[test]
    fn test_handle_body() {
        let reactors = mock_reactors();

        let bytes = encode_bytes(&b"cici hunter".to_vec(), Method::Vote);
        let ctx = Context::new();

        let maybe_ok = block_on(InboundHandle::handle_body(reactors, ctx, bytes));
        assert_eq!(maybe_ok.unwrap(), ());
    }

    #[test]
    fn test_handle_body_with_bad_data() {
        let reactors = mock_reactors();

        let bytes = Bytes::from(b"bad bytes".to_vec());
        let ctx = Context::new();

        match block_on(InboundHandle::handle_body(reactors, ctx, bytes)) {
            Err(Error::MsgCodecError(NetMessageError::DecodeError(_))) => (), // pass
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[test]
    fn test_handle_body_with_wrong_size_data() {
        let reactors = mock_reactors();

        let bytes = mock_wrong_size_data();
        let ctx = Context::new();

        match block_on(InboundHandle::handle_body(reactors, ctx, bytes)) {
            Err(Error::IoError(err)) => {
                assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
                assert!(format!("{:?}", err.get_ref()).contains("data corruption"));
            }
            _ => panic!("should return Error::IoError"),
        }
    }

    #[test]
    fn test_handle_body_with_wrong_method_data() {
        let reactors = mock_reactors();

        let bytes = mock_wrong_method_data();
        let ctx = Context::new();

        match block_on(InboundHandle::handle_body(reactors, ctx, bytes)) {
            Err(Error::UnknownMethod(m)) => assert_eq!(m, 9_999_999),
            _ => panic!("should return Error::UnknownMethod"),
        }
    }

    #[test]
    fn test_handle_body_with_unknown_method() {
        let reactors = mock_reactors();

        let bytes = encode_bytes(&b"hameihameiha".to_vec(), Method::SyncPullTxs);
        let ctx = Context::new();

        match block_on(InboundHandle::handle_body(reactors, ctx, bytes)) {
            Err(Error::UnknownMethod(m)) => assert_eq!(m, Method::SyncPullTxs.to_u32()),
            _ => panic!("should return Error::UnknownMethod"),
        }
    }

    #[test]
    fn test_handle_body_with_reactor_error() {
        let reactors = mock_reactors();

        let bytes = encode_bytes(&b"13".to_vec(), Method::PullTxs);
        let ctx = Context::new();

        match block_on(InboundHandle::handle_body(reactors, ctx, bytes)) {
            Err(Error::UnknownMethod(m)) => assert_eq!(m, 9_999_999),
            _ => panic!("should return Error::UnknownMethod"),
        }
    }

    #[test]
    fn test_handle_msg() {
        let (inbound, _) = mock_inbound();

        let bytes = encode_bytes(&b"cici hunter".to_vec(), Method::Vote);
        let sess_msg = mock_sess_msg(bytes);

        assert_eq!(block_on(inbound.handle_msg(sess_msg)), ());
    }

    // TODO: test logger
    #[test]
    fn test_handle_msg_with_bad_data() {
        let (inbound, _) = mock_inbound();

        let bytes = Bytes::from(b"bad bytes".to_vec());
        let sess_msg = mock_sess_msg(bytes);

        assert_eq!(block_on(inbound.handle_msg(sess_msg)), ());
    }

    #[test]
    fn test_inbound_stop() {
        let (mut inbound, tx) = mock_inbound();
        drop(tx);

        assert_eq!(block_on(inbound.next()), None);
    }
}
