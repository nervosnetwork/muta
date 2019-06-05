mod consensus;
mod sync;
mod tx_pool;

use std::sync::Arc;
use std::time::Duration;

use futures::prelude::{FutureExt, StreamExt};
use futures::select;
use futures_timer::Delay;
use log::error;

use common_channel::Receiver;
use core_context::Context;
use core_network_message::{Codec, Message, Method};

use crate::callback_map::{CallId, Callback};
use crate::common::session_id_from_context;
use crate::config::DEFAULT_RPC_TIMEOUT;
use crate::p2p::{Bytes, Outbound, Scope};
use crate::Error;

const CALL_ID_KEY: &str = "net_call_id";

pub trait BytesBroadcaster {
    fn broadcast(&self, scope: Scope, bytes: Bytes) -> Result<(), Error>;
    fn quick_broadcast(&self, scope: Scope, bytes: Bytes) -> Result<(), Error>;
}

pub trait CallbackChannel {
    fn new_call_id(&self) -> CallId;
    fn make<T: 'static>(&self, call_id: u64, sess_id: usize) -> Receiver<T>;
}

pub trait DataEncoder {
    fn encode(&self, method: Method) -> Result<Bytes, Error>;
}

#[derive(PartialEq, Eq, Debug)]
pub enum Mode {
    Normal,
    Quick,
}

pub struct OutboundHandle<B, C> {
    broadcaster: B,
    cb_chan:     Arc<C>,

    rpc_timeout: u64,
}

impl<B, C> OutboundHandle<B, C> {
    pub fn new(broadcaster: B, cb_chan: Arc<C>, rpc_timeout: Option<u64>) -> Self {
        let rpc_timeout = rpc_timeout.unwrap_or_else(|| DEFAULT_RPC_TIMEOUT);

        OutboundHandle {
            broadcaster,
            cb_chan,
            rpc_timeout,
        }
    }
}

impl<B, C> OutboundHandle<B, C>
where
    B: BytesBroadcaster,
{
    pub fn broadcast<D>(&self, method: Method, data: D, scope: Scope) -> Result<(), Error>
    where
        D: DataEncoder,
    {
        let bytes = data.encode(method)?;
        self.broadcaster.broadcast(scope, bytes)?;

        Ok(())
    }

    pub fn quick_broadcast<D>(&self, method: Method, data: D, scope: Scope) -> Result<(), Error>
    where
        D: DataEncoder,
    {
        let bytes = data.encode(method)?;
        self.broadcaster.quick_broadcast(scope, bytes)?;

        Ok(())
    }

    pub fn silent_broadcast<D>(&self, method: Method, data: D, mode: Mode)
    where
        D: DataEncoder,
    {
        if let Err(err) = match mode {
            Mode::Normal => self.broadcast(method, data, Scope::All),
            Mode::Quick => self.quick_broadcast(method, data, Scope::All),
        } {
            error!("net [out]: [method: {:?}, err: {:?}]", method, err);
        }
    }
}

impl<B, C> OutboundHandle<B, C>
where
    B: BytesBroadcaster,
    C: CallbackChannel,
{
    pub async fn rpc<D, R>(self, ctx: Context, method: Method, data: D) -> Result<R, String>
    where
        D: DataEncoder,
        R: 'static,
    {
        let cb_chan = Arc::clone(&self.cb_chan);
        let rpc_timeout = self.rpc_timeout;

        // NOTE: Not found means fata logic error
        let call_id = {
            let ref_id = ctx.get::<CallId>(CALL_ID_KEY);
            ref_id.expect("call id should be set before rpc").to_owned()
        };

        let sess_err = |_| format!("net [out]: [method: {:?}]: session id not found", method);
        let sess_id = session_id_from_context(&ctx).map_err(sess_err)?;
        let scope = Scope::Single(sess_id);

        self.quick_broadcast(method, data, scope)
            .map_err(|err| format!("net [out]: [method {:?}, err: {:?}]", method, err))?;

        let mut timeout = Delay::new(Duration::from_secs(rpc_timeout)).fuse();
        let mut done_rx = cb_chan.make::<R>(call_id.value(), sess_id.value()).fuse();
        let done_err = || format!("net [out]: [method: {:?}]: done_rx return None", method);

        // FIXME: report fail to set up timeout error upward
        select! {
            opt_resp = done_rx.next() => opt_resp.ok_or_else(done_err),
            maybe_timeout = timeout => match maybe_timeout {
                Ok(_) => Err("net [out]: [method: {:?}] timeout".to_owned()),
                Err(err) => Err("net [out]: set up timeout failure".to_owned()),
            }
        }
    }
}

impl<B, C> Clone for OutboundHandle<B, C>
where
    B: BytesBroadcaster + Clone,
{
    fn clone(&self) -> Self {
        OutboundHandle {
            broadcaster: self.broadcaster.clone(),
            cb_chan:     Arc::clone(&self.cb_chan),
            rpc_timeout: self.rpc_timeout,
        }
    }
}

impl BytesBroadcaster for Outbound {
    fn broadcast(&self, scope: Scope, bytes: Bytes) -> Result<(), Error> {
        self.filter_broadcast(scope, bytes)
    }

    fn quick_broadcast(&self, scope: Scope, bytes: Bytes) -> Result<(), Error> {
        self.quick_filter_broadcast(scope, bytes)
    }
}

impl CallbackChannel for Callback {
    fn new_call_id(&self) -> CallId {
        self.new_call_id()
    }

    fn make<T: 'static>(&self, call_id: u64, sess_id: usize) -> Receiver<T> {
        self.insert::<T>(call_id, sess_id)
    }
}

impl<T> DataEncoder for T
where
    T: Codec,
    Error: From<<T as Codec>::Error>,
{
    fn encode(&self, method: Method) -> Result<Bytes, Error> {
        let encoded = self.encode()?;

        let msg = Message {
            method:    method.to_u32(),
            data_size: encoded.len() as u64,
            data:      encoded.to_vec(),
        };

        Ok(<Message as Codec>::encode(&msg)?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::{any::Any, io};

    use futures::executor::block_on;

    use common_channel::{bounded, Receiver, Sender};
    use core_context::{Context, P2P_SESSION_ID};
    use core_network_message::{Codec, Message, Method};

    use crate::callback_map::CallId;
    use crate::p2p::{Bytes, ConnectionError, Scope, SessionId};
    use crate::Error;

    use super::{
        BytesBroadcaster, CallbackChannel, DataEncoder, Mode, OutboundHandle, CALL_ID_KEY,
    };

    // TODO: https://github.com/myelin-ai/mockiato/issues/113
    #[derive(Clone)]
    pub struct MockBroadcaster {
        reply_err: Arc<AtomicBool>,
        bytes:     Arc<Mutex<Option<(Mode, Scope, Bytes)>>>,
    }

    impl MockBroadcaster {
        pub fn new() -> Self {
            MockBroadcaster {
                reply_err: Arc::new(AtomicBool::new(false)),
                bytes:     Arc::new(Mutex::new(None)),
            }
        }

        pub fn reply_err(&self, switch: bool) {
            self.reply_err.store(switch, Ordering::Relaxed);
        }

        pub fn broadcasted_bytes(&self) -> Option<(Mode, Scope, Bytes)> {
            self.bytes.lock().unwrap().take()
        }

        fn save_bytes(&self, scope: Scope, bytes: Bytes, mode: Mode) -> Result<(), Error> {
            if self.reply_err.load(Ordering::Relaxed) {
                Err(self.error())
            } else {
                *self.bytes.lock().unwrap() = Some((mode, scope, bytes));
                Ok(())
            }
        }

        fn error(&self) -> Error {
            let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "mock err");

            Error::ConnectionError(ConnectionError::IoError(io_err))
        }
    }

    impl BytesBroadcaster for MockBroadcaster {
        fn broadcast(&self, scope: Scope, bytes: Bytes) -> Result<(), Error> {
            self.save_bytes(scope, bytes, Mode::Normal)
        }

        fn quick_broadcast(&self, scope: Scope, bytes: Bytes) -> Result<(), Error> {
            self.save_bytes(scope, bytes, Mode::Quick)
        }
    }

    unsafe impl Send for MockBroadcaster {}
    unsafe impl Sync for MockBroadcaster {}

    // TODO: https://github.com/myelin-ai/mockiato/issues/117
    pub struct MockCallbackChannel {
        rx: Mutex<Option<Box<dyn Any + 'static>>>,
    }

    impl MockCallbackChannel {
        pub fn new<T: 'static>() -> (Self, Sender<T>) {
            let (tx, rx) = bounded(1);

            let cb_chan = MockCallbackChannel {
                rx: Mutex::new(Some(Box::new(rx) as Box<dyn Any + 'static>)),
            };

            (cb_chan, tx)
        }
    }

    impl CallbackChannel for MockCallbackChannel {
        fn new_call_id(&self) -> CallId {
            CallId::new(1)
        }

        fn make<T: 'static>(&self, _: u64, _: usize) -> Receiver<T> {
            let mut opt_rx = self.rx.lock().unwrap();
            let box_rx = opt_rx.take().unwrap().downcast::<Receiver<T>>().unwrap();

            *box_rx
        }
    }

    unsafe impl Send for MockCallbackChannel {}
    unsafe impl Sync for MockCallbackChannel {}

    struct BadData;

    impl Codec for BadData {
        type Error = Error;

        fn encode(&self) -> Result<Bytes, Self::Error> {
            let msg_err = core_network_message::Error::UnknownMethod(1);

            Err(Error::MsgCodecError(msg_err))
        }

        fn decode(_raw: &[u8]) -> Result<Self, Self::Error> {
            unimplemented!()
        }
    }

    type MockOutboundHandle = OutboundHandle<MockBroadcaster, MockCallbackChannel>;

    pub fn new_outbound<T: 'static>() -> (MockOutboundHandle, Sender<T>) {
        let (cb_chan, tx) = MockCallbackChannel::new::<T>();
        let broadcaster = MockBroadcaster::new();
        let rpc_timeout = Some(1); // 1 seconds

        (
            OutboundHandle::new(broadcaster, Arc::new(cb_chan), rpc_timeout),
            tx,
        )
    }

    pub fn encode_bytes<T: DataEncoder>(data: &T, method: Method) -> Bytes {
        <T as DataEncoder>::encode(&data, method).unwrap()
    }

    #[test]
    fn test_data_encoder_encode() {
        let data = b"night city 2077".to_vec();
        let data_bytes = <Vec<u8> as Codec>::encode(&data).unwrap();
        let msg_bytes = encode_bytes(&data, Method::Vote);

        let msg = <Message as Codec>::decode(&msg_bytes).unwrap();

        assert_eq!(msg.method, Method::Vote.to_u32());
        assert_eq!(msg.data_size, data_bytes.len() as u64);
        assert_eq!(msg.data, data_bytes);
    }

    #[test]
    fn test_data_encoder_with_bad_data() {
        use core_network_message::Error as NetMessageError;

        match <BadData as DataEncoder>::encode(&BadData, Method::Vote) {
            Err(Error::MsgCodecError(NetMessageError::UnknownMethod(_))) => (),
            _ => panic!("should return Error::MsgCodecError"),
        }
    }

    #[test]
    fn test_broadcast() {
        let data = b"infinite lhoa 2020".to_vec();
        let msg_bytes = encode_bytes(&data, Method::Vote);

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcast(Method::Vote, data, Scope::All).unwrap();

        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Normal, Scope::All, msg_bytes))
        );
    }

    #[test]
    fn test_broadcast_with_bad_data() {
        let (outbound, _) = new_outbound::<()>();

        match outbound.broadcast(Method::Vote, BadData, Scope::All) {
            Err(Error::MsgCodecError(_)) => (), // passed
            _ => panic!("should return MsgCodecError"),
        }
    }

    #[test]
    fn test_broadcast_but_fail() {
        let data = b"dalin john".to_vec();
        let (outbound, _) = new_outbound::<()>();

        outbound.broadcaster.reply_err(true);
        match outbound.broadcast(Method::Proposal, data, Scope::All) {
            Err(Error::ConnectionError(ConnectionError::IoError(_))) => (), // passed
            _ => panic!("should return ConnectionError"),
        }
    }

    #[test]
    fn test_quick_broadcast() {
        let data = b"the last night".to_vec();
        let method = Method::Proposal;
        let msg_bytes = encode_bytes(&data, method);

        let (outbound, _) = new_outbound::<()>();

        outbound.quick_broadcast(method, data, Scope::All).unwrap();
        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, Scope::All, msg_bytes))
        );
    }

    #[test]
    fn test_quick_broadcast_with_bad_data() {
        let (outbound, _) = new_outbound::<()>();

        match outbound.broadcast(Method::Proposal, BadData, Scope::All) {
            Err(Error::MsgCodecError(_)) => (), // passed
            _ => panic!("should return MsgCodecError"),
        }
    }

    #[test]
    fn test_quick_broadcast_but_fail() {
        let data = b"7FF chapter 1".to_vec();
        let (outbound, _) = new_outbound::<()>();

        outbound.broadcaster.reply_err(true);
        match outbound.broadcast(Method::Proposal, data, Scope::All) {
            Err(Error::ConnectionError(ConnectionError::IoError(_))) => (), // passed
            _ => panic!("should return ConnectionError"),
        }
    }

    #[test]
    fn test_silent_broadcast() {
        let data = b"7FF chapter 2".to_vec();
        let method = Method::Proposal;
        let msg_bytes = encode_bytes(&data, method);

        let (outbound, _) = new_outbound::<()>();
        outbound.silent_broadcast(method, data.clone(), Mode::Normal);
        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Normal, Scope::All, msg_bytes.clone()))
        );

        outbound.silent_broadcast(method, data.clone(), Mode::Quick);
        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, Scope::All, msg_bytes))
        );

        outbound.broadcaster.reply_err(true);
        assert_eq!(outbound.broadcaster.broadcasted_bytes(), None);
    }

    #[test]
    fn test_rpc() {
        let data = b"son of sun".to_vec();
        let method = Method::Proposal;
        let msg_bytes = encode_bytes(&data, method);
        let scope = Scope::Single(SessionId::new(1));

        let ctx = Context::new()
            .with_value(P2P_SESSION_ID, 1)
            .with_value::<CallId>(CALL_ID_KEY, CallId::new(1));
        let (outbound, done_tx) = new_outbound::<Vec<u8>>();

        let expect_resp = b"you".to_vec();
        done_tx.try_send(expect_resp.clone()).unwrap();
        let resp: Vec<u8> = block_on(outbound.clone().rpc(ctx, Method::Proposal, data)).unwrap();

        assert_eq!(resp, expect_resp);
        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, scope, msg_bytes))
        );
    }

    #[test]
    #[should_panic(expected = "call id should be set before rpc")]
    fn test_rpc_panic_without_ctx_call_id() {
        let data = b"fish same".to_vec();
        let ctx = Context::new();
        let (outbound, _) = new_outbound::<()>();

        block_on(outbound.rpc::<Vec<u8>, Vec<u8>>(ctx, Method::Proposal, data)).unwrap();
    }

    #[test]
    fn test_rpc_without_ctx_session_id() {
        let data = b"shell out of ghost".to_vec();
        let ctx = Context::new().with_value::<CallId>(CALL_ID_KEY, CallId::new(1));
        let (outbound, _) = new_outbound::<()>();

        match block_on(outbound.rpc::<Vec<u8>, Vec<u8>>(ctx, Method::Proposal, data)) {
            Err(str) => assert!(str.contains("session id not found")),
            _ => panic!("should return error string contains 'session id not found'"),
        }
    }

    #[test]
    fn test_rpc_with_broadcast_failure() {
        let data = b"taqikema".to_vec();
        let ctx = Context::new()
            .with_value(P2P_SESSION_ID, 1)
            .with_value::<CallId>(CALL_ID_KEY, CallId::new(1));

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcaster.reply_err(true);

        match block_on(outbound.rpc::<Vec<u8>, Vec<u8>>(ctx, Method::Vote, data)) {
            Err(str) => assert!(str.contains("mock err")),
            _ => panic!("should return error string contains 'mock err'"),
        }
    }

    #[test]
    fn test_rpc_with_disconnected_done_channel() {
        let data = b"taqikema".to_vec();
        let ctx = Context::new()
            .with_value(P2P_SESSION_ID, 1)
            .with_value::<CallId>(CALL_ID_KEY, CallId::new(1));

        let (outbound, done_tx) = new_outbound::<Vec<u8>>();
        drop(done_tx);

        match block_on(outbound.rpc::<Vec<u8>, Vec<u8>>(ctx, Method::Vote, data)) {
            Err(str) => assert!(str.contains("done_rx return None")),
            _ => panic!("should return error string contains 'done_rx return None'"),
        }
    }

    #[test]
    fn test_rpc_timeout() {
        let data = b"death strand".to_vec();
        let ctx = Context::new()
            .with_value(P2P_SESSION_ID, 1)
            .with_value::<CallId>(CALL_ID_KEY, CallId::new(1));

        // hold _done_tx, but do not send anything
        let (outbound, _done_tx) = new_outbound::<Vec<u8>>();

        match block_on(outbound.rpc::<Vec<u8>, Vec<u8>>(ctx, Method::Vote, data)) {
            Err(str) => assert!(str.contains("timeout")),
            _ => panic!("should return error string contains 'timeout'"),
        }
    }
}
