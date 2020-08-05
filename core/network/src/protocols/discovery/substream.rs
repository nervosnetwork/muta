use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use bytes::{BufMut, BytesMut};
use futures::channel::mpsc::Receiver;
use futures::lock::Mutex;
use futures::{Sink, Stream};
use log::{debug, trace, warn};
use prost::Message;
use tentacle::context::ProtocolContextMutRef;
use tentacle::error::SendErrorKind;
use tentacle::multiaddr::{Multiaddr, Protocol};
use tentacle::service::{ServiceControl, SessionType};
use tentacle::utils::multiaddr_to_socketaddr;
use tentacle::{ProtocolId, SessionId};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{length_delimited::LengthDelimitedCodec, Decoder, Encoder, Framed};

use crate::protocols::identify::WaitIdentification;

use super::addr::{AddrKnown, AddressManager, ConnectableAddr, Misbehavior};
use super::message::{DiscoveryMessage, Nodes, Payload};

// FIXME: should be a more high level version number
const VERSION: u32 = 0;
// The maximum number of new addresses to accumulate before announcing.
const MAX_ADDR_TO_SEND: u32 = 1000;
// Every 24 hours send announce nodes message
const ANNOUNCE_INTERVAL: u64 = 3600 * 24;
const ANNOUNCE_THRESHOLD: usize = 10;

// The maximum number addresses in on Nodes item
const MAX_ADDRS: usize = 3;

pub(crate) struct DiscoveryCodec {
    inner: LengthDelimitedCodec,
}

impl Default for DiscoveryCodec {
    fn default() -> DiscoveryCodec {
        DiscoveryCodec {
            inner: LengthDelimitedCodec::new(),
        }
    }
}

impl Decoder for DiscoveryCodec {
    type Error = io::Error;
    type Item = DiscoveryMessage;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.inner.decode(src) {
            Ok(Some(frame)) => {
                let maybe_msg = DiscoveryMessage::decode(frame.freeze());
                maybe_msg.map(Some).map_err(|err| {
                    debug!("deserialize {}", err);
                    io::ErrorKind::InvalidData.into()
                })
            }
            Ok(None) => Ok(None),
            Err(err) => {
                debug!("codec decode {}", err);
                Err(io::ErrorKind::InvalidData.into())
            }
        }
    }
}

impl Encoder for DiscoveryCodec {
    type Error = io::Error;
    type Item = DiscoveryMessage;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mut buf = BytesMut::with_capacity(item.encoded_len());
        item.encode(&mut buf).map_err(|err| {
            warn!("serialize {}", err);
            io::ErrorKind::InvalidData
        })?;

        self.inner.encode(buf.freeze(), dst)
    }
}

pub enum IdentifyStatus {
    Done(Result<(), ()>),
    Wait(WaitIdentification),
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct SubstreamKey {
    pub(crate) direction:  SessionType,
    pub(crate) session_id: SessionId,
    pub(crate) proto_id:   ProtocolId,
}

pub struct StreamHandle {
    data_buf:            BytesMut,
    proto_id:            ProtocolId,
    session_id:          SessionId,
    pub(crate) receiver: Receiver<Vec<u8>>,
    pub(crate) sender:   ServiceControl,
}

impl AsyncRead for StreamHandle {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        for _ in 0..10 {
            match Pin::new(&mut self.receiver).as_mut().poll_next(cx) {
                Poll::Ready(Some(data)) => {
                    self.data_buf.reserve(data.len());
                    self.data_buf.put(data.as_slice());
                }
                Poll::Ready(None) => {
                    return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
                }
                Poll::Pending => {
                    break;
                }
            }
        }
        let n = std::cmp::min(buf.len(), self.data_buf.len());
        if n == 0 {
            return Poll::Pending;
        }
        let b = self.data_buf.split_to(n);
        buf[..n].copy_from_slice(&b);
        Poll::Ready(Ok(n))
    }
}

impl AsyncWrite for StreamHandle {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.sender
            .send_message_to(self.session_id, self.proto_id, BytesMut::from(buf).freeze())
            .map(|()| buf.len())
            .map_err(|e| match e {
                SendErrorKind::WouldBlock => io::ErrorKind::WouldBlock.into(),
                SendErrorKind::BrokenPipe => io::ErrorKind::BrokenPipe.into(),
            })
            .into()
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

pub struct SubstreamValue {
    framed_stream:                  Framed<StreamHandle, DiscoveryCodec>,
    // received pending messages
    pub(crate) pending_messages:    VecDeque<DiscoveryMessage>,
    pub(crate) addr_known:          AddrKnown,
    // FIXME: Remote listen address, resolved by id protocol
    pub(crate) remote_addr:         RemoteAddress,
    pub(crate) announce:            bool,
    pub(crate) last_announce:       Option<Instant>,
    pub(crate) announce_multiaddrs: Vec<Multiaddr>,
    session_id:                     SessionId,
    announce_interval:              Duration,
    received_get_nodes:             bool,
    received_nodes:                 bool,
    remote_closed:                  bool,
    pub(crate) identify_status:     Arc<Mutex<IdentifyStatus>>,
}

impl SubstreamValue {
    pub(crate) fn new(
        direction: SessionType,
        substream: Substream,
        max_known: usize,
        query_cycle: Option<Duration>,
    ) -> SubstreamValue {
        let session_id = substream.stream.session_id;
        let mut pending_messages = VecDeque::default();
        debug!("direction: {:?}", direction);
        let mut addr_known = AddrKnown::new(max_known);
        let remote_addr = if direction.is_outbound() {
            pending_messages.push_back(DiscoveryMessage::new_get_nodes(
                VERSION,
                MAX_ADDR_TO_SEND,
                substream.listen_port,
            ));
            addr_known.insert(ConnectableAddr::from(&substream.remote_addr));

            RemoteAddress::Listen(substream.remote_addr)
        } else {
            RemoteAddress::Init(substream.remote_addr)
        };

        let identify_status = Arc::new(Mutex::new(IdentifyStatus::Wait(substream.ident_fut)));
        SubstreamValue {
            framed_stream: Framed::new(substream.stream, DiscoveryCodec::default()),
            last_announce: None,
            announce_interval: query_cycle
                .unwrap_or_else(|| Duration::from_secs(ANNOUNCE_INTERVAL)),
            pending_messages,
            addr_known,
            remote_addr,
            session_id,
            announce: false,
            announce_multiaddrs: Vec::new(),
            received_get_nodes: false,
            received_nodes: false,
            remote_closed: false,
            identify_status,
        }
    }

    fn remote_connectable_addr(&self) -> ConnectableAddr {
        ConnectableAddr::from(self.remote_addr.to_inner())
    }

    pub(crate) fn check_timer(&mut self) {
        if self
            .last_announce
            .map(|time| time.elapsed() > self.announce_interval)
            .unwrap_or(true)
        {
            debug!("announce this session: {:?}", self.session_id);
            self.announce = true;
        }
    }

    pub(crate) fn send_messages(&mut self, cx: &mut Context) -> Result<(), io::Error> {
        let mut sink = Pin::new(&mut self.framed_stream);

        while let Some(message) = self.pending_messages.pop_front() {
            debug!("Discovery sending message: {}", message);

            match sink.as_mut().poll_ready(cx)? {
                Poll::Pending => {
                    self.pending_messages.push_front(message);
                    return Ok(());
                }
                Poll::Ready(()) => {
                    sink.as_mut().start_send(message)?;
                }
            }
        }
        let _ = sink.as_mut().poll_flush(cx)?;
        Ok(())
    }

    pub(crate) fn handle_message(
        &mut self,
        message: DiscoveryMessage,
        addr_mgr: &mut AddressManager,
    ) -> Result<Option<Nodes>, io::Error> {
        match message {
            DiscoveryMessage {
                payload: Some(Payload::GetNodes(get_nodes)),
            } => {
                if self.received_get_nodes {
                    // TODO: misbehavior
                    if addr_mgr
                        .misbehave(self.session_id, Misbehavior::DuplicateGetNodes)
                        .is_disconnect()
                    {
                        // TODO: more clear error type
                        warn!("Already received get nodes");
                        return Err(io::ErrorKind::Other.into());
                    }
                } else {
                    // TODO: magic number
                    // must get the item first, otherwise it is possible to load
                    // the address of peer listen.
                    let mut items = addr_mgr.get_random(2500, self.session_id);

                    // change client random outbound port to client listen port
                    let listen_port = get_nodes.listen_port();
                    debug!("listen port: {:?}", listen_port);
                    if let Some(port) = listen_port {
                        self.remote_addr.update_port(port);
                        self.addr_known.insert(self.remote_connectable_addr());

                        // add client listen address to manager
                        if let RemoteAddress::Listen(ref addr) = self.remote_addr {
                            let identify_status = Arc::clone(&self.identify_status);
                            let session_id = self.session_id;
                            let addr = addr.clone();
                            let mut addr_mgr = addr_mgr.clone();

                            tokio::spawn(async move {
                                let mut status = identify_status.lock().await;

                                match &mut *status {
                                    IdentifyStatus::Done(ret) => {
                                        if ret.is_err() {
                                            return;
                                        }
                                        addr_mgr.add_new_addr(session_id, addr);
                                    }
                                    IdentifyStatus::Wait(fut) => {
                                        let ret = fut.await;
                                        if ret.is_ok() {
                                            addr_mgr.add_new_addr(session_id, addr);
                                        }
                                        *status = IdentifyStatus::Done(ret);
                                    }
                                }
                            });
                        }
                    }

                    while items.len() > 1000 {
                        if let Some(last_item) = items.pop() {
                            let idx = rand::random::<usize>() % 1000;
                            items[idx] = last_item;
                        }
                    }

                    let announce = false;
                    let items = items.into_iter().map(|addr| vec![addr]).collect::<Vec<_>>();

                    self.pending_messages
                        .push_back(DiscoveryMessage::new_nodes(announce, items));

                    self.received_get_nodes = true;
                }
            }
            DiscoveryMessage {
                payload: Some(Payload::Nodes(nodes)),
            } => {
                for item in &nodes.items {
                    if item.addrs.len() > MAX_ADDRS {
                        let misbehavior = Misbehavior::TooManyAddresses(item.addrs.len());
                        if addr_mgr
                            .misbehave(self.session_id, misbehavior)
                            .is_disconnect()
                        {
                            // TODO: more clear error type
                            return Err(io::ErrorKind::Other.into());
                        }
                    }
                }

                if nodes.announce {
                    if nodes.items.len() > ANNOUNCE_THRESHOLD {
                        warn!("Nodes items more than {}", ANNOUNCE_THRESHOLD);
                        // TODO: misbehavior
                        let misbehavior = Misbehavior::TooManyItems {
                            announce: nodes.announce,
                            length:   nodes.items.len(),
                        };
                        if addr_mgr
                            .misbehave(self.session_id, misbehavior)
                            .is_disconnect()
                        {
                            // TODO: more clear error type
                            return Err(io::ErrorKind::Other.into());
                        }
                    } else {
                        return Ok(Some(nodes));
                    }
                } else if self.received_nodes {
                    warn!("already received Nodes(announce=false) message");
                    // TODO: misbehavior
                    if addr_mgr
                        .misbehave(self.session_id, Misbehavior::DuplicateFirstNodes)
                        .is_disconnect()
                    {
                        // TODO: more clear error type
                        return Err(io::ErrorKind::Other.into());
                    }
                } else if nodes.items.len() > MAX_ADDR_TO_SEND as usize {
                    warn!(
                        "Too many items (announce=false) length={}",
                        nodes.items.len()
                    );
                    // TODO: misbehavior
                    let misbehavior = Misbehavior::TooManyItems {
                        announce: nodes.announce,
                        length:   nodes.items.len(),
                    };

                    if addr_mgr
                        .misbehave(self.session_id, misbehavior)
                        .is_disconnect()
                    {
                        // TODO: more clear error type
                        return Err(io::ErrorKind::Other.into());
                    }
                } else {
                    self.received_nodes = true;
                    return Ok(Some(nodes));
                }
            }
            DiscoveryMessage { payload: None } => {
                // TODO: misbehavior
            }
        }
        Ok(None)
    }

    pub(crate) fn receive_messages(
        &mut self,
        cx: &mut Context,
        addr_mgr: &mut AddressManager,
    ) -> Result<Option<(SessionId, Vec<Nodes>)>, io::Error> {
        if self.remote_closed {
            return Ok(None);
        }

        let mut nodes_list = Vec::new();
        loop {
            match Pin::new(&mut self.framed_stream).as_mut().poll_next(cx) {
                Poll::Ready(Some(res)) => {
                    let message = res?;
                    trace!("received message {}", message);
                    if let Some(nodes) = self.handle_message(message, addr_mgr)? {
                        // Add to known address list
                        for node in &nodes.items {
                            for addr in node.clone().addrs() {
                                trace!("received address: {}", addr);
                                self.addr_known.insert(ConnectableAddr::from(addr));
                            }
                        }
                        nodes_list.push(nodes);
                    }
                }
                Poll::Ready(None) => {
                    debug!("remote closed");
                    self.remote_closed = true;
                    break;
                }
                Poll::Pending => {
                    break;
                }
            }
        }
        Ok(Some((self.session_id, nodes_list)))
    }
}

pub struct Substream {
    pub remote_addr: Multiaddr,
    pub direction:   SessionType,
    pub stream:      StreamHandle,
    pub listen_port: Option<u16>,
    pub ident_fut:   WaitIdentification,
}

impl Substream {
    pub fn new(
        ident_fut: WaitIdentification,
        context: ProtocolContextMutRef,
        receiver: Receiver<Vec<u8>>,
    ) -> Substream {
        let stream = StreamHandle {
            data_buf: BytesMut::default(),
            proto_id: context.proto_id,
            session_id: context.session.id,
            receiver,
            sender: context.control().clone(),
        };
        let listen_port = if context.session.ty.is_outbound() {
            context
                .listens()
                .iter()
                .map(|address| multiaddr_to_socketaddr(address).unwrap().port())
                .next()
        } else {
            None
        };
        Substream {
            remote_addr: context.session.address.clone(),
            direction: context.session.ty,
            stream,
            listen_port,
            ident_fut,
        }
    }

    pub fn key(&self) -> SubstreamKey {
        SubstreamKey {
            direction:  self.direction,
            session_id: self.stream.session_id,
            proto_id:   self.stream.proto_id,
        }
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub(crate) enum RemoteAddress {
    /// Inbound init remote address
    Init(Multiaddr),
    /// Outbound init remote address or Inbound listen address
    Listen(Multiaddr),
}

impl RemoteAddress {
    fn to_inner(&self) -> &Multiaddr {
        match self {
            RemoteAddress::Init(ref addr) | RemoteAddress::Listen(ref addr) => addr,
        }
    }

    pub(crate) fn into_inner(self) -> Multiaddr {
        match self {
            RemoteAddress::Init(addr) | RemoteAddress::Listen(addr) => addr,
        }
    }

    fn update_port(&mut self, port: u16) {
        if let RemoteAddress::Init(ref addr) = self {
            let addr = addr
                .into_iter()
                .map(|proto| {
                    match proto {
                        // TODO: other transport, UDP for example
                        Protocol::TCP(_) => Protocol::TCP(port),
                        value => value,
                    }
                })
                .collect();
            *self = RemoteAddress::Listen(addr);
        }
    }
}
