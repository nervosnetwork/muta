use super::message::{PingMessage, PingPayload};

use futures::channel::mpsc::Sender;
use log::{debug, error, warn};
use prost::Message;
use tentacle::{
    context::{ProtocolContext, ProtocolContextMutRef},
    secio::PeerId,
    service::TargetSession,
    traits::ServiceProtocol,
    SessionId,
};

use std::{
    collections::HashMap,
    str,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const SEND_PING_TOKEN: u64 = 0;
const CHECK_TIMEOUT_TOKEN: u64 = 1;

/// Ping protocol events
#[derive(Debug)]
pub enum PingEvent {
    /// Peer send ping to us.
    Ping(PeerId),
    /// Peer send pong to us.
    Pong(PeerId, Duration),
    /// Peer is timeout.
    Timeout(PeerId),
    /// Peer cause a unexpected error.
    UnexpectedError(PeerId),
}

/// PingStatus of a peer
#[derive(Clone, Debug)]
struct PingStatus {
    /// Are we currently pinging this peer?
    processing: bool,
    /// The time we last send ping to this peer.
    last_ping:  SystemTime,
    peer_id:    PeerId,
}

impl PingStatus {
    /// A meaningless value, peer must send a pong has same nonce to respond a
    /// ping.
    fn nonce(&self) -> u32 {
        self.last_ping
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_secs())
            .unwrap_or(0) as u32
    }

    /// Time duration since we last send ping.
    fn elapsed(&self) -> Duration {
        self.last_ping.elapsed().unwrap_or(Duration::from_secs(0))
    }
}

/// Ping protocol handler.
///
/// The interval means that we send ping to peers.
/// The timeout means that consider peer is timeout if during a timeout we still
/// have not received pong from a peer
pub struct PingProtocol {
    interval:              Duration,
    timeout:               Duration,
    connected_session_ids: HashMap<SessionId, PingStatus>,
    event_sender:          Sender<PingEvent>,
}

impl PingProtocol {
    pub fn new(
        interval: Duration,
        timeout: Duration,
        event_sender: Sender<PingEvent>,
    ) -> PingProtocol {
        PingProtocol {
            interval,
            timeout,
            connected_session_ids: Default::default(),
            event_sender,
        }
    }

    pub fn send_event(&mut self, event: PingEvent) {
        if let Err(err) = self.event_sender.try_send(event) {
            error!("send ping event error: {}", err);
        }
    }
}

impl ServiceProtocol for PingProtocol {
    fn init(&mut self, context: &mut ProtocolContext) {
        // send ping to peers periodically
        let proto_id = context.proto_id;
        if context
            .set_service_notify(proto_id, self.interval, SEND_PING_TOKEN)
            .is_err()
        {
            warn!("start ping fail");
        }
        if context
            .set_service_notify(proto_id, self.timeout, CHECK_TIMEOUT_TOKEN)
            .is_err()
        {
            warn!("start ping fail");
        }
    }

    fn connected(&mut self, context: ProtocolContextMutRef, version: &str) {
        let session = context.session;
        match session.remote_pubkey {
            Some(ref pubkey) => {
                let peer_id = pubkey.peer_id();
                self.connected_session_ids
                    .entry(session.id)
                    .or_insert_with(|| PingStatus {
                        last_ping:  SystemTime::now(),
                        processing: false,
                        peer_id:    peer_id.clone(),
                    });
                debug!(
                    "proto id [{}] open on session [{}], address: [{}], type: [{:?}], version: {}",
                    context.proto_id, session.id, session.address, session.ty, version
                );
                debug!("connected sessions are: {:?}", self.connected_session_ids);

                crate::protocols::OpenedProtocols::register(peer_id, context.proto_id);
            }
            None => {
                if context.disconnect(session.id).is_err() {
                    debug!("disconnect fail");
                }
            }
        }
    }

    fn disconnected(&mut self, context: ProtocolContextMutRef) {
        let session = context.session;
        self.connected_session_ids.remove(&session.id);
        debug!(
            "proto id [{}] close on session [{}]",
            context.proto_id, session.id
        );
    }

    fn received(&mut self, context: ProtocolContextMutRef, data: bytes::Bytes) {
        let session = context.session;
        if let Some(peer_id) = self
            .connected_session_ids
            .get(&session.id)
            .map(|ps| ps.peer_id.clone())
        {
            match PingMessage::decode(data) {
                Err(err) => {
                    warn!("decode message {}", err);
                    self.send_event(PingEvent::UnexpectedError(peer_id))
                }
                Ok(PingMessage { payload: None }) => {
                    self.send_event(PingEvent::UnexpectedError(peer_id))
                }
                Ok(PingMessage { payload: Some(pld) }) => match pld {
                    PingPayload::Ping(nonce) => {
                        let pong = match PingMessage::new_pong(nonce).into_bytes() {
                            Ok(p) => p,
                            Err(err) => {
                                warn!("encode pong {}", err);
                                return;
                            }
                        };

                        if let Err(err) = context.send_message(pong) {
                            debug!("send message {}", err);
                        }
                        self.send_event(PingEvent::Ping(peer_id));
                    }
                    PingPayload::Pong(nonce) => {
                        // check pong
                        if self
                            .connected_session_ids
                            .get(&session.id)
                            .map(|ps| (ps.processing, ps.nonce()))
                            == Some((true, nonce))
                        {
                            let ping_time = match self.connected_session_ids.get_mut(&session.id) {
                                Some(ps) => {
                                    ps.processing = false;
                                    ps.elapsed()
                                }
                                None => return,
                            };
                            self.send_event(PingEvent::Pong(peer_id, ping_time));
                        } else {
                            // ignore if nonce is incorrect

                            self.send_event(PingEvent::UnexpectedError(peer_id));
                        }
                    }
                },
            }
        }
    }

    fn notify(&mut self, context: &mut ProtocolContext, token: u64) {
        match token {
            SEND_PING_TOKEN => {
                debug!("proto [{}] start ping peers", context.proto_id);
                let now = SystemTime::now();
                let peers: Vec<(SessionId, u32)> = self
                    .connected_session_ids
                    .iter_mut()
                    .filter_map(|(session_id, ps)| {
                        if ps.processing {
                            None
                        } else {
                            ps.processing = true;
                            ps.last_ping = now;
                            Some((*session_id, ps.nonce()))
                        }
                    })
                    .collect();
                if !peers.is_empty() {
                    let ping = match PingMessage::new_ping(peers[0].1).into_bytes() {
                        Ok(p) => p,
                        Err(err) => {
                            warn!("encode ping {}", err);
                            return;
                        }
                    };

                    let peer_ids: Vec<SessionId> = peers
                        .into_iter()
                        .map(|(session_id, _)| session_id)
                        .collect();
                    let proto_id = context.proto_id;
                    let target = TargetSession::Multi(peer_ids);

                    if let Err(err) = context.filter_broadcast(target, proto_id, ping) {
                        debug!("send message {}", err);
                    }
                }
            }
            CHECK_TIMEOUT_TOKEN => {
                debug!("proto [{}] check ping timeout", context.proto_id);
                let timeout = self.timeout;
                for peer_id in self
                    .connected_session_ids
                    .values()
                    .filter(|ps| ps.processing && ps.elapsed() >= timeout)
                    .map(|ps| ps.peer_id.clone())
                    .collect::<Vec<PeerId>>()
                {
                    self.send_event(PingEvent::Timeout(peer_id));
                }
            }
            _ => panic!("unknown token {}", token),
        }
    }
}
