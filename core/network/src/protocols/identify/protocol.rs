use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use derive_more::Display;
use futures::future::{self, AbortHandle};
use futures_timer::Delay;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use prost::Message;
use protocol::Bytes;
use tentacle::multiaddr::{Multiaddr, Protocol};
use tentacle::secio::PeerId;
use tentacle::service::{SessionType, TargetProtocol};
use tentacle::traits::SessionProtocol;
use tentacle::{ProtocolId, SessionId};

#[cfg(test)]
use crate::test::mock::{ServiceControl, SessionContext};
#[cfg(not(test))]
use tentacle::context::{ProtocolContextMutRef, SessionContext};
#[cfg(not(test))]
use tentacle::service::ServiceControl;

#[cfg(not(test))]
use super::behaviour::IdentifyBehaviour;
#[cfg(test)]
use super::tests::MockIdentifyBehaviour;

use super::identification::{Identification, WaitIdentification};
use super::message::{Acknowledge, AddressInfoMessage, Identity};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(8);
pub const MAX_MESSAGE_SIZE: usize = 5 * 1000; // 5KB

lazy_static! {
    // NOTE: Use peer id here because trust metric integrated test run in one process
    static ref PEER_IDENTIFICATION_BACKLOG: RwLock<HashMap<PeerId, Identification>> =
        RwLock::new(HashMap::new());
}

#[derive(Debug, Display, Clone)]
pub enum Error {
    #[display(fmt = "wrong chain id")]
    WrongChainId,

    #[display(fmt = "timeout")]
    Timeout,

    #[display(fmt = "exceed max message size")]
    ExceedMaxMessageSize,

    #[display(fmt = "decode indentity failed")]
    DecodeIdentityFailed,

    #[display(fmt = "decode ack failed")]
    DecodeAckFailed,

    #[display(fmt = "{}", _0)]
    InvalidMessage(String),

    #[display(fmt = "wait future dropped")]
    WaitFutDropped,

    #[display(fmt = "disconnected")]
    Disconnected,

    #[display(fmt = "{}", _0)]
    Other(String),
}

// Wrap ProtocolContextMutRef for easy mock and test
#[cfg(not(test))]
pub struct IdentifyProtocolContext<'a>(ProtocolContextMutRef<'a>);
#[cfg(test)]
pub struct IdentifyProtocolContext<'a>(pub &'a crate::test::mock::ProtocolContext);

#[derive(Debug, Display)]
#[display(fmt = "peer {:?} addr {:?}", id, addr)]
pub struct RemotePeer {
    pub id:   PeerId,
    pub sid:  SessionId,
    pub addr: Multiaddr,
}

pub struct NoEncryption;

impl RemotePeer {
    pub fn from_proto_context(
        proto_context: &IdentifyProtocolContext,
    ) -> Result<RemotePeer, NoEncryption> {
        match proto_context.0.session.remote_pubkey.as_ref() {
            None => Err(NoEncryption),
            Some(pubkey) => {
                let remote_peer = RemotePeer {
                    id:   pubkey.peer_id(),
                    sid:  proto_context.0.session.id,
                    addr: proto_context.0.session.address.to_owned(),
                };

                Ok(remote_peer)
            }
        }
    }
}

pub struct StateContext {
    pub remote_peer:          Arc<RemotePeer>,
    pub proto_id:             ProtocolId,
    pub service_control:      ServiceControl,
    pub session_context:      SessionContext,
    pub timeout_abort_handle: Option<AbortHandle>,
}

impl StateContext {
    pub fn from_proto_context(
        proto_context: &IdentifyProtocolContext,
    ) -> Result<StateContext, NoEncryption> {
        let remote_peer = RemotePeer::from_proto_context(proto_context)?;
        let state_context = StateContext {
            remote_peer:          Arc::new(remote_peer),
            proto_id:             proto_context.0.proto_id(),
            service_control:      proto_context.0.control().clone(),
            session_context:      proto_context.0.session.clone(),
            timeout_abort_handle: None,
        };

        Ok(state_context)
    }

    pub fn observed_addr(&self) -> Multiaddr {
        let remote_addr = self.session_context.address.iter();

        remote_addr
            .filter(|proto| match proto {
                Protocol::P2P(_) => false,
                _ => true,
            })
            .collect()
    }

    pub fn send_message(&self, msg: Bytes) {
        if let Err(err) =
            self.service_control
                .quick_send_message_to(self.remote_peer.sid, self.proto_id, msg)
        {
            log::warn!(
                "internal error: quick send message to {} failed {}",
                self.remote_peer,
                err
            );
        }
    }

    pub fn disconnect(&self) {
        let _ = self.service_control.disconnect(self.remote_peer.sid);
    }

    pub fn open_protocols(&self) {
        if let Err(err) = self
            .service_control
            .open_protocols(self.remote_peer.sid, TargetProtocol::All)
        {
            log::warn!("open protocols to peer {} failed {}", self.remote_peer, err);
            self.disconnect()
        }
    }

    pub fn set_open_protocols_timeout(&mut self, timeout: Duration) {
        let service_control = self.service_control.clone();
        let remote_peer = Arc::clone(&self.remote_peer);

        tokio::spawn(async move {
            Delay::new(timeout).await;

            if crate::protocols::OpenedProtocols::is_all_opened(&remote_peer.id) {
                return;
            }

            log::warn!("peer {} open protocols timeout, disconnect it", remote_peer);
            let _ = service_control.disconnect(remote_peer.sid);
        });
    }

    pub fn set_timeout(&mut self, description: &'static str, timeout: Duration) {
        let service_control = self.service_control.clone();
        let remote_peer = Arc::clone(&self.remote_peer);

        let (timeout, timeout_abort_handle) = future::abortable(async move {
            Delay::new(timeout).await;

            log::warn!(
                "{} timeout from peer {}, disconnect it",
                description,
                remote_peer,
            );

            finish_identify(&remote_peer, Err(self::Error::Timeout));
            let _ = service_control.disconnect(remote_peer.sid);
        });

        self.timeout_abort_handle = Some(timeout_abort_handle);
        tokio::spawn(timeout);
    }

    pub fn cancel_timeout(&self) {
        if let Some(timeout) = self.timeout_abort_handle.as_ref() {
            timeout.abort()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum ClientProcedure {
    #[display(fmt = "client wait for server identity acknowledge")]
    WaitAck,

    #[display(fmt = "client open other protocols")]
    OpenOtherProtocols,

    #[display(fmt = "server failed identification")]
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum ServerProcedure {
    #[display(fmt = "server wait for client identity")]
    WaitIdentity,

    #[display(fmt = "server wait for client open protocols")]
    WaitOpenProtocols, // After accept session

    #[display(fmt = "client failed identification")]
    Failed,
}

pub enum State {
    SessionProtocolInited,
    FailedWithoutEncryption,
    FailedWithExceedMsgSize,
    ClientNegotiate {
        procedure: ClientProcedure,
        context:   StateContext,
    },
    ServerNegotiate {
        procedure: ServerProcedure,
        context:   StateContext,
    },
}

pub struct IdentifyProtocol {
    pub(crate) state:     State,
    #[cfg(not(test))]
    behaviour:            Arc<IdentifyBehaviour>,
    #[cfg(test)]
    pub(crate) behaviour: Arc<MockIdentifyBehaviour>,
}

impl IdentifyProtocol {
    #[cfg(not(test))]
    pub fn new(behaviour: Arc<IdentifyBehaviour>) -> Self {
        IdentifyProtocol {
            state: State::SessionProtocolInited,
            behaviour,
        }
    }

    #[cfg(test)]
    pub fn new() -> Self {
        IdentifyProtocol {
            state:     State::SessionProtocolInited,
            behaviour: Arc::new(MockIdentifyBehaviour::new()),
        }
    }

    pub fn wait(peer_id: PeerId) -> WaitIdentification {
        let mut backlog = PEER_IDENTIFICATION_BACKLOG.write();
        let identification = backlog.entry(peer_id).or_insert_with(Identification::new);

        identification.wait()
    }

    pub fn wait_failed(peer_id: &PeerId, error: String) {
        if let Some(identification) = { PEER_IDENTIFICATION_BACKLOG.write().remove(peer_id) } {
            identification.failed(self::Error::Other(error))
        }
    }

    pub fn on_connected(&mut self, protocol_context: &IdentifyProtocolContext) {
        let mut state_context = match StateContext::from_proto_context(protocol_context) {
            Ok(ctx) => ctx,
            Err(_no) => {
                // Without peer id, there's no way to register a wait identification.No
                // need to clean it.
                log::warn!(
                    "session from {:?} without encryption, disconnect it",
                    protocol_context.0.session.address
                );

                self.state = State::FailedWithoutEncryption;
                let _ = protocol_context.0.disconnect(protocol_context.0.session.id);
                return;
            }
        };
        log::debug!("connected from {:?}", state_context.remote_peer);

        crate::protocols::OpenedProtocols::register(
            state_context.remote_peer.id.to_owned(),
            state_context.proto_id,
        );

        match protocol_context.0.session.ty {
            SessionType::Inbound => {
                state_context.set_timeout("wait client identity", DEFAULT_TIMEOUT);

                self.state = State::ServerNegotiate {
                    procedure: ServerProcedure::WaitIdentity,
                    context:   state_context,
                };
            }
            SessionType::Outbound => {
                self.behaviour.send_identity(&state_context);
                state_context.set_timeout("wait server ack", DEFAULT_TIMEOUT);

                self.state = State::ClientNegotiate {
                    procedure: ClientProcedure::WaitAck,
                    context:   state_context,
                };
            }
        }
    }

    pub fn on_disconnected(&mut self, protocol_context: &IdentifyProtocolContext) {
        // Without peer id, there's no way to register a wait identification. No
        // need to clean it.
        let peer_id = match protocol_context.0.session.remote_pubkey.as_ref() {
            Some(pubkey) => pubkey.peer_id(),
            None => return,
        };

        // TODO: Remove from upper level
        crate::protocols::OpenedProtocols::remove(&peer_id);

        if let Some(identification) = PEER_IDENTIFICATION_BACKLOG.write().remove(&peer_id) {
            identification.failed(self::Error::Disconnected);
        }
    }

    pub fn on_received(&mut self, protocol_context: &IdentifyProtocolContext, data: Bytes) {
        {
            if data.len() > MAX_MESSAGE_SIZE {
                let peer_id = match protocol_context.0.session.remote_pubkey.as_ref() {
                    Some(pubkey) => pubkey.peer_id(),
                    None => return,
                };

                if let Some(identification) = PEER_IDENTIFICATION_BACKLOG.write().remove(&peer_id) {
                    identification.failed(self::Error::ExceedMaxMessageSize);
                    self.state = State::FailedWithExceedMsgSize;
                    let _ = protocol_context.0.disconnect(protocol_context.0.session.id);
                    return;
                }
            }
        }

        match &mut self.state {
            State::ServerNegotiate {
                ref mut procedure,
                context,
            } => match procedure {
                ServerProcedure::WaitIdentity => {
                    let identity = match Identity::decode(data) {
                        Ok(ident) => ident,
                        Err(_) => {
                            log::warn!("received invalid identity from {:?}", context.remote_peer);

                            finish_identify(
                                &context.remote_peer,
                                Err(self::Error::DecodeIdentityFailed),
                            );
                            *procedure = ServerProcedure::Failed;
                            context.disconnect();
                            return;
                        }
                    };
                    context.cancel_timeout();

                    if let Err(err) = identity.validate() {
                        finish_identify(
                            &context.remote_peer,
                            Err(self::Error::InvalidMessage(err.to_string())),
                        );
                        *procedure = ServerProcedure::Failed;
                        context.disconnect();
                        return;
                    }

                    if let Err(err) = self.behaviour.verify_remote_identity(&identity) {
                        finish_identify(&context.remote_peer, Err(err));
                        *procedure = ServerProcedure::Failed;
                        context.disconnect();
                        return;
                    }

                    finish_identify(&context.remote_peer, Ok(()));

                    let listen_addrs = identity.addr_info.listen_addrs();
                    self.behaviour.process_listens(&context, listen_addrs);

                    if let Some(observed_addr) = identity.addr_info.observed_addr() {
                        self.behaviour.process_observed(&context, observed_addr);
                    }

                    self.behaviour.send_ack(&context);
                    context.set_open_protocols_timeout(DEFAULT_TIMEOUT);
                    *procedure = ServerProcedure::WaitOpenProtocols;
                }
                ServerProcedure::Failed | ServerProcedure::WaitOpenProtocols => {
                    log::warn!(
                        "should not received any more message from {} after acked identity",
                        context.remote_peer
                    );
                    context.disconnect();
                }
            },
            State::ClientNegotiate {
                ref mut procedure,
                context,
            } => match procedure {
                ClientProcedure::WaitAck => {
                    let acknowledge = match Acknowledge::decode(data) {
                        Ok(ack) => ack,
                        Err(_) => {
                            log::warn!("received invalid ack from {:?}", context.remote_peer);

                            finish_identify(
                                &context.remote_peer,
                                Err(self::Error::DecodeAckFailed),
                            );
                            *procedure = ClientProcedure::Failed;
                            context.disconnect();
                            return;
                        }
                    };
                    context.cancel_timeout();

                    if let Err(err) = acknowledge.validate() {
                        finish_identify(
                            &context.remote_peer,
                            Err(self::Error::InvalidMessage(err.to_string())),
                        );
                        *procedure = ClientProcedure::Failed;
                        context.disconnect();
                        return;
                    }

                    finish_identify(&context.remote_peer, Ok(()));

                    let listen_addrs = acknowledge.addr_info.listen_addrs();
                    self.behaviour.process_listens(&context, listen_addrs);

                    if let Some(observed_addr) = acknowledge.addr_info.observed_addr() {
                        self.behaviour.process_observed(&context, observed_addr);
                    }

                    context.open_protocols();
                    *procedure = ClientProcedure::OpenOtherProtocols;
                }
                ClientProcedure::OpenOtherProtocols | ClientProcedure::Failed => {
                    log::warn!(
                        "should not received any more message from {} after open protocols",
                        context.remote_peer
                    );
                    context.disconnect();
                }
            },
            _ => {
                log::warn!(
                    "should not received message from {} out of negotiate state",
                    protocol_context.0.session.address
                );
                let _ = protocol_context.0.disconnect(protocol_context.0.session.id);
            }
        }
    }
}

#[cfg(test)]
impl SessionProtocol for IdentifyProtocol {}

#[cfg(not(test))]
impl SessionProtocol for IdentifyProtocol {
    fn connected(&mut self, protocol_context: ProtocolContextMutRef, _version: &str) {
        self.on_connected(&IdentifyProtocolContext(protocol_context));
    }

    fn disconnected(&mut self, protocol_context: ProtocolContextMutRef) {
        self.on_disconnected(&IdentifyProtocolContext(protocol_context));
    }

    fn received(&mut self, protocol_context: ProtocolContextMutRef, data: bytes::Bytes) {
        self.on_received(&IdentifyProtocolContext(protocol_context), data)
    }
}

fn finish_identify(peer: &RemotePeer, result: Result<(), self::Error>) {
    let identification = match { PEER_IDENTIFICATION_BACKLOG.write().remove(&peer.id) } {
        Some(ident) => ident,
        None => {
            log::debug!("peer {:?} identification has finished already", peer);
            return;
        }
    };

    match result {
        Ok(()) => identification.pass(),
        Err(err) => {
            log::warn!("identification for peer {} failed: {}", peer, err);
            identification.failed(err);
        }
    }
}
