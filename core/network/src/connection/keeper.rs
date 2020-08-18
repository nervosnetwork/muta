use std::sync::atomic::{AtomicBool, Ordering};

use futures::channel::mpsc::UnboundedSender;
use log::{debug, error};
use tentacle::secio::error::SecioError;
use tentacle::{
    context::ServiceContext,
    error::{DialerErrorKind, HandshakeErrorKind, ListenErrorKind},
    multiaddr::Multiaddr,
    service::{ServiceError, ServiceEvent},
    traits::ServiceHandle,
};

use crate::{
    error::{ErrorKind, NetworkError},
    event::{
        ConnectionErrorKind, ConnectionType, PeerManagerEvent, ProtocolIdentity, SessionErrorKind,
    },
};

#[cfg(test)]
use crate::test::mock::SessionContext;

// This macro tries to extract PublicKey from SessionContext, it's Optional.
// If it get None, then simple `return` to exit caller function. Otherwise,
// return PublicKey reference.
macro_rules! peer_pubkey {
    ($session_context:expr) => {{
        let opt_pk = $session_context.remote_pubkey.as_ref();
        debug_assert!(opt_pk.is_some(), "secio is enforced, no way it's None here");

        if let Some(pubkey) = opt_pk {
            pubkey
        } else {
            return;
        }
    }};
}

pub struct ConnectionServiceKeeper {
    peer_mgr:     UnboundedSender<PeerManagerEvent>,
    sys_reporter: UnboundedSender<NetworkError>,

    sys_shutdown: AtomicBool,
}

impl ConnectionServiceKeeper {
    pub fn new(
        peer_mgr: UnboundedSender<PeerManagerEvent>,
        sys_reporter: UnboundedSender<NetworkError>,
    ) -> Self {
        ConnectionServiceKeeper {
            peer_mgr,
            sys_reporter,

            sys_shutdown: AtomicBool::new(false),
        }
    }

    fn is_sys_shutdown(&self) -> bool {
        self.sys_shutdown.load(Ordering::SeqCst)
    }

    fn sys_shutdown(&self) {
        self.sys_shutdown.store(true, Ordering::SeqCst);
    }

    fn report_error(&self, kind: ErrorKind) {
        debug!("network: connection error: {}", kind);

        if !self.is_sys_shutdown() {
            let error = NetworkError::from(kind);

            if self.sys_reporter.unbounded_send(error).is_err() {
                error!("network: connection: error report channel dropped");

                self.sys_shutdown();
            }
        }
    }

    fn report_peer(&self, event: PeerManagerEvent) {
        if self.peer_mgr.unbounded_send(event).is_err() {
            self.report_error(ErrorKind::Offline("peer manager"));
        }
    }

    fn process_dailer_error(&self, addr: Multiaddr, error: DialerErrorKind) {
        use DialerErrorKind::{
            HandshakeError, IoError, PeerIdNotMatch, RepeatedConnection, TransportError,
        };

        let kind = match error {
            IoError(err) => ConnectionErrorKind::Io(err),
            PeerIdNotMatch => ConnectionErrorKind::PeerIdNotMatch,
            RepeatedConnection(sid) => {
                let ty = ConnectionType::Outbound;
                let repeated_connection = PeerManagerEvent::RepeatedConnection { ty, sid, addr };
                return self.report_peer(repeated_connection);
            }
            HandshakeError(HandshakeErrorKind::Timeout(reason)) => {
                ConnectionErrorKind::TimeOut(reason)
            }
            HandshakeError(HandshakeErrorKind::SecioError(SecioError::IoError(err))) => {
                ConnectionErrorKind::Io(err)
            }
            HandshakeError(err) => ConnectionErrorKind::SecioHandshake(Box::new(err)),
            TransportError(err) => ConnectionErrorKind::from(err),
        };

        let dail_failed = PeerManagerEvent::ConnectFailed { addr, kind };
        self.report_peer(dail_failed);
    }

    fn process_listen_error(&self, addr: Multiaddr, error: ListenErrorKind) {
        use ListenErrorKind::{IoError, RepeatedConnection, TransportError};

        let kind = match error {
            IoError(err) => ConnectionErrorKind::Io(err),
            RepeatedConnection(sid) => {
                let ty = ConnectionType::Outbound;
                let repeated_connection = PeerManagerEvent::RepeatedConnection { ty, sid, addr };
                return self.report_peer(repeated_connection);
            }
            TransportError(err) => ConnectionErrorKind::from(err),
        };

        let listen_failed = PeerManagerEvent::ConnectFailed { addr, kind };
        self.report_peer(listen_failed);
    }
}

#[rustfmt::skip]
impl ServiceHandle for ConnectionServiceKeeper {
    fn handle_error(&mut self, _ctx: &mut ServiceContext, err: ServiceError) {
        match err {
            ServiceError::DialerError { error, address } => {
                self.process_dailer_error(address, error)
            }
            ServiceError::ListenError { error, address } => {
                self.process_listen_error(address, error)
            }
            ServiceError::ProtocolSelectError { session_context, proto_name } => {
                let protocol_identity = if let Some(proto_name) = proto_name {
                    Some(ProtocolIdentity::Name(proto_name))
                } else {
                    None
                };

                let kind = SessionErrorKind::Protocol {
                    identity: protocol_identity,
                    cause: None,
                };

                let protocol_select_failure = PeerManagerEvent::SessionFailed {
                    sid: session_context.id,
                    kind,
                };

                self.report_peer(protocol_select_failure);
            }

            ServiceError::ProtocolError { id, error, proto_id } => {
                let kind = SessionErrorKind::Protocol {
                    identity: Some(ProtocolIdentity::Id(proto_id)),
                    cause: Some(Box::new(error)),
                };
                let broken_protocol = PeerManagerEvent::SessionFailed { sid: id, kind };

                self.report_peer(broken_protocol);
            }

            ServiceError::SessionTimeout { session_context } => {
                let kind = SessionErrorKind::Io(std::io::ErrorKind::TimedOut.into());
                let session_timeout = PeerManagerEvent::SessionFailed {
                    sid: session_context.id,
                    kind,
                };

                self.report_peer(session_timeout);
            }

            ServiceError::MuxerError { session_context, error } => {
                let muxer_broken = PeerManagerEvent::SessionFailed {
                    sid: session_context.id,
                    kind: SessionErrorKind::Io(error)
                };

                self.report_peer(muxer_broken);
            }

            // Bad protocol code, will cause memory leaks/abnormal CPU usage
            ServiceError::ProtocolHandleError { error, proto_id } => {
                error!("network: bad protocol {} implement: {}", proto_id, error);

                let kind = ErrorKind::BadProtocolHandle {proto_id, cause : Box::new(error)};
                self.report_error(kind);
            }

            // Partial protocol task logic take long time to process, usually
            // indicate bad protocol implement.
            ServiceError::SessionBlocked { session_context } => {
                #[cfg(test)]
                let session_context = SessionContext::from(session_context).arced();

                let session_blocked = PeerManagerEvent::SessionBlocked {
                    ctx: session_context
                };
                self.report_peer(session_blocked);
            }
        }
    }

    fn handle_event(&mut self, ctx: &mut ServiceContext, evt: ServiceEvent) {
        match evt {
            ServiceEvent::SessionOpen { session_context } => {
                if session_context.remote_pubkey.is_none() {
                    // Peer without encryption will not be able to connect to us
                    error!("impossible, got connection from/to {:?} without public key, disconnect it", session_context.address);

                    // Just in case
                    if let Err(e) = ctx.disconnect(session_context.id) {
                        error!("disconnect session {} {}", session_context.id, e);
                    }
                    return;
                }

                let pubkey = peer_pubkey!(&session_context).clone();
                let pid = pubkey.peer_id();
                #[cfg(test)]
                let session_context = SessionContext::from(session_context).arced();
                let new_unidentified_session = PeerManagerEvent::UnidentifiedSession { pid, pubkey, ctx: session_context };

                self.report_peer(new_unidentified_session);
            }
            ServiceEvent::SessionClose { session_context } => {
                let pid = peer_pubkey!(&session_context).peer_id();
                let sid = session_context.id;

                let peer_session_closed = PeerManagerEvent::SessionClosed { pid, sid };

                self.report_peer(peer_session_closed);
            }
            ServiceEvent::ListenStarted { address } => {
                let start_listen = PeerManagerEvent::AddNewListenAddr { addr: address };

                self.report_peer(start_listen);
            }
            ServiceEvent::ListenClose { address } => {
                let close_listen = PeerManagerEvent::RemoveListenAddr { addr: address };

                self.report_peer(close_listen);
            }
        }
    }
}
