use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use derive_more::Display;
use futures::future::{self, AbortHandle};
use futures_timer::Delay;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use prost::Message;
use tentacle::context::{ProtocolContextMutRef, SessionContext};
use tentacle::multiaddr::{Multiaddr, Protocol};
use tentacle::secio::PeerId;
use tentacle::service::{ServiceControl, SessionType, TargetProtocol};
use tentacle::traits::SessionProtocol;

use super::behaviour::{IdentifyBehaviour, Misbehavior, MAX_ADDRS};
use super::common::reachable;
use super::identification::{Identification, WaitIdentification};
use super::message::{self, Acknowledge, Identity};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(8);

lazy_static! {
    // NOTE: Use peer id here because trust metric integrated test run in one process
    static ref PEER_IDENTIFICATION_BACKLOG: RwLock<HashMap<PeerId, Identification>> =
        RwLock::new(HashMap::new());
}

#[derive(Debug, Display, Clone)]
pub enum Error {
    #[display(fmt = "remote peer does not enable encryption")]
    EncryptionNotEnabled,

    #[display(fmt = "wrong identity {}", _0)]
    WrongIdentity(String),

    #[display(fmt = "timeout")]
    Timeout,

    #[display(fmt = "wait future dropped")]
    WaitFutDropped,

    #[display(fmt = "disconnected")]
    Disconnected,
}

pub struct ProcedureContext {
    pub peer_id:              PeerId,
    pub service_control:      ServiceControl,
    pub session_context:      SessionContext,
    pub timeout_abort_handle: Option<AbortHandle>,
}

impl ProcedureContext {
    pub fn cancel_timeout(&self) {
        if let Some(timeout) = self.timeout_abort_handle.as_ref() {
            timeout.abort()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientProcedure {
    WaitAck,
    OpenOtherProtocols,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerProcedure {
    WaitIdentity,
    AckedIdentity, // After accept session
}

pub enum Procedure {
    New,
    Client {
        current: ClientProcedure,
        context: ProcedureContext,
    },
    Server {
        current: ServerProcedure,
        context: ProcedureContext,
    },
}

pub struct IdentifyProtocol {
    procedure: Procedure,
    behaviour: Arc<IdentifyBehaviour>,
}

impl IdentifyProtocol {
    pub fn new(behaviour: Arc<IdentifyBehaviour>) -> Self {
        IdentifyProtocol {
            procedure: Procedure::New,
            behaviour,
        }
    }

    pub fn wait(peer_id: PeerId) -> WaitIdentification {
        let identification = Identification::new();
        let wait_fut = identification.wait();

        {
            PEER_IDENTIFICATION_BACKLOG
                .write()
                .insert(peer_id, identification);
        }

        wait_fut
    }

    fn new_procedure_context(
        &mut self,
        context: &ProtocolContextMutRef,
    ) -> Result<ProcedureContext, self::Error> {
        let session = context.session;
        let remote_peer_id = match &session.remote_pubkey {
            Some(pubkey) => pubkey.peer_id(),
            None => {
                return Err(self::Error::EncryptionNotEnabled);
            }
        };

        let procedure_context = ProcedureContext {
            peer_id:              remote_peer_id,
            service_control:      context.control().clone(),
            session_context:      context.session.clone(),
            timeout_abort_handle: None,
        };

        Ok(procedure_context)
    }

    pub fn listen_addrs(behaviour: &IdentifyBehaviour) -> Vec<Multiaddr> {
        behaviour
            .local_listen_addrs()
            .into_iter()
            .filter(reachable)
            .take(MAX_ADDRS)
            .collect()
    }

    pub fn observed_addr(session_context: &SessionContext) -> Multiaddr {
        session_context
            .address
            .iter()
            .filter(|proto| match proto {
                Protocol::P2P(_) => false,
                _ => true,
            })
            .collect::<Multiaddr>()
    }
}

impl SessionProtocol for IdentifyProtocol {
    fn connected(&mut self, context: ProtocolContextMutRef, _version: &str) {
        let mut procedure_context = match self.new_procedure_context(&context) {
            Ok(c) => c,
            Err(err) => {
                log::warn!("create procedure context failed: {}", err);
                let _ = context.disconnect(context.session.id);
                return;
            }
        };
        log::debug!("connected from {:?}", procedure_context.peer_id);

        let service_control = procedure_context.service_control.clone();
        let session_context = procedure_context.session_context.clone();
        match context.session.ty {
            SessionType::Inbound => {
                let (timeout, timeout_abort_handle) = future::abortable(async move {
                    Delay::new(DEFAULT_TIMEOUT).await;
                    log::warn!(
                        "wait identity from session {} timeout, disconnect it",
                        session_context.id
                    );
                    let _ = service_control.disconnect(session_context.id);
                });
                procedure_context.timeout_abort_handle = Some(timeout_abort_handle);
                tokio::spawn(timeout);

                self.procedure = Procedure::Server {
                    current: ServerProcedure::WaitIdentity,
                    context: procedure_context,
                };
            }
            SessionType::Outbound => {
                let identity = self.behaviour.identity();
                let address_info = {
                    let listen_addrs = Self::listen_addrs(&self.behaviour);
                    let observed_addr = Self::observed_addr(&context.session);
                    message::AddressInfo::new(listen_addrs, observed_addr)
                };

                let init_msg = match Identity::new(identity, address_info).into_bytes() {
                    Ok(msg) => msg,
                    Err(err) => {
                        log::warn!("encode {}", err);
                        let _ = service_control.disconnect(session_context.id);
                        return;
                    }
                };

                if let Err(err) = context.quick_send_message(init_msg) {
                    log::warn!("quick send message {}", err);
                }

                let (timeout, timeout_abort_handle) = future::abortable(async move {
                    Delay::new(DEFAULT_TIMEOUT).await;
                    log::warn!(
                        "wait acknowledge from session {} timeout, disconnect it",
                        session_context.id
                    );
                    let _ = service_control.disconnect(session_context.id);
                });
                procedure_context.timeout_abort_handle = Some(timeout_abort_handle);
                tokio::spawn(timeout);

                self.procedure = Procedure::Client {
                    current: ClientProcedure::WaitAck,
                    context: procedure_context,
                };
            }
        }
    }

    fn disconnected(&mut self, context: ProtocolContextMutRef) {
        let peer_id = match context.session.remote_pubkey.as_ref() {
            Some(pubkey) => pubkey.peer_id(),
            None => return,
        };

        if let Some(identification) = PEER_IDENTIFICATION_BACKLOG.write().remove(&peer_id) {
            identification.failed(self::Error::Disconnected);
        }
    }

    fn received(&mut self, protocol_context: ProtocolContextMutRef, data: bytes::Bytes) {
        let session = protocol_context.session;

        match &mut self.procedure {
            Procedure::Server {
                ref mut current,
                context,
            } => {
                match current {
                    ServerProcedure::WaitIdentity => {
                        match Identity::decode(data) {
                            Ok(msg) => {
                                context.cancel_timeout();

                                let behaviour = &mut self.behaviour;
                                let wait_identified = match {
                                    PEER_IDENTIFICATION_BACKLOG.write().remove(&context.peer_id)
                                } {
                                    Some(ident) => ident,
                                    None => {
                                        let _ = protocol_context.disconnect(session.id);
                                        return;
                                    }
                                };

                                // Need to interrupt processing, avoid pollution
                                if behaviour
                                    .received_identity(
                                        &context.peer_id,
                                        &wait_identified,
                                        &msg.identity,
                                    )
                                    .is_disconnect()
                                {
                                    let _ = protocol_context.disconnect(session.id);
                                    return;
                                }

                                if behaviour
                                    .process_listens(&context, msg.listen_addrs())
                                    .is_disconnect()
                                    || behaviour
                                        .process_observed(&context, msg.observed_addr())
                                        .is_disconnect()
                                {
                                    let _ = protocol_context.disconnect(session.id);
                                    return;
                                }

                                let address_info = {
                                    let listen_addrs = Self::listen_addrs(&self.behaviour);
                                    let observed_addr = Self::observed_addr(&session);
                                    message::AddressInfo::new(listen_addrs, observed_addr)
                                };
                                let init_msg = match Acknowledge::new(address_info).into_bytes() {
                                    Ok(msg) => msg,
                                    Err(err) => {
                                        log::warn!("encode {}", err);
                                        let _ = protocol_context
                                            .disconnect(protocol_context.session.id);
                                        return;
                                    }
                                };

                                if let Err(err) = protocol_context.quick_send_message(init_msg) {
                                    log::warn!("quick send message {}", err);
                                }

                                *current = ServerProcedure::AckedIdentity;
                            }
                            Err(_) => {
                                log::warn!("received invalid data from {:?}", context.peer_id);

                                if self
                                    .behaviour
                                    .misbehave(&context.peer_id, Misbehavior::InvalidData)
                                    .is_disconnect()
                                {
                                    let _ = protocol_context.disconnect(session.id);
                                }
                            }
                        }
                    }
                    ServerProcedure::AckedIdentity => {
                        // TODO: misbehave duplicate data
                        log::warn!("receive duplicate data from peer {:?}", context.peer_id);
                    }
                }
            }
            Procedure::Client {
                ref mut current,
                context,
            } => {
                match current {
                    ClientProcedure::WaitAck => match Acknowledge::decode(data) {
                        Ok(msg) => {
                            context.cancel_timeout();
                            log::warn!("received server ack");

                            let behaviour = &mut self.behaviour;
                            let identification = match {
                                PEER_IDENTIFICATION_BACKLOG.write().remove(&context.peer_id)
                            } {
                                Some(ident) => ident,
                                None => {
                                    let _ = protocol_context.disconnect(session.id);
                                    return;
                                }
                            };
                            identification.pass();

                            if behaviour
                                .process_listens(&context, msg.listen_addrs())
                                .is_disconnect()
                                || behaviour
                                    .process_observed(&context, msg.observed_addr())
                                    .is_disconnect()
                            {
                                let _ = protocol_context.disconnect(session.id);
                                return;
                            }

                            if let Err(err) = protocol_context
                                .open_protocols(protocol_context.session.id, TargetProtocol::All)
                            {
                                log::warn!("open protocols {}", err);
                                let _ = protocol_context.disconnect(protocol_context.session.id);
                                return;
                            }

                            *current = ClientProcedure::OpenOtherProtocols;
                        }
                        Err(_) => {
                            log::warn!("received invalid data from {:?}", context.peer_id);

                            if self
                                .behaviour
                                .misbehave(&context.peer_id, Misbehavior::InvalidData)
                                .is_disconnect()
                            {
                                let _ = protocol_context.disconnect(session.id);
                            }
                        }
                    },
                    ClientProcedure::OpenOtherProtocols => {
                        // TODO: misbehave init identify
                        log::warn!(
                            "client receive data during init identify from peer {:?}",
                            context.peer_id
                        );
                    }
                };
            }
            Procedure::New => unreachable!(),
        }
    }
}
