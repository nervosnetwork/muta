use std::sync::atomic::{AtomicBool, Ordering};

use futures::channel::mpsc::UnboundedSender;
use log::{debug, error};
use tentacle::{
    context::ServiceContext,
    error::Error as TentacleError,
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

    fn process_connect_error(&self, ty: ConnectionType, error: TentacleError, addr: Multiaddr) {
        use TentacleError::*;

        let kind = match error {
            ConnectSelf => {
                let connect_self = PeerManagerEvent::AddNewListenAddr { addr };
                return self.report_peer(connect_self);
            }
            RepeatedConnection(sid) => {
                let repeated_connection = PeerManagerEvent::RepeatedConnection { ty, sid, addr };
                return self.report_peer(repeated_connection);
            }
            IoError(err) => ConnectionErrorKind::Io(err),
            PeerIdNotMatch => ConnectionErrorKind::PeerIdNotMatch,
            DNSResolverError(e) => ConnectionErrorKind::DNSResolver(Box::new(e)),
            HandshakeError(e) => ConnectionErrorKind::SecioHandshake(Box::new(e)),
            SessionProtoHandleBlock(_)
            | ServiceProtoHandleBlock
            | ServiceProtoHandleAbnormallyClosed
            | SessionProtoHandleAbnormallyClosed(_) => ConnectionErrorKind::ProtocolHandle,
        };

        let connect_failed = PeerManagerEvent::ConnectFailed { addr, kind };
        self.report_peer(connect_failed);
    }
}

#[rustfmt::skip]
impl ServiceHandle for ConnectionServiceKeeper {
    fn handle_error(&mut self, _ctx: &mut ServiceContext, err: ServiceError) {
        match err {
            ServiceError::DialerError { error, address } => {
                self.process_connect_error(ConnectionType::Outbound, error, address)
            }
            ServiceError::ListenError { error, address } => {
                self.process_connect_error(ConnectionType::Inbound, error, address)
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
                let kind = if let TentacleError::IoError(err) = error {
                    SessionErrorKind::Io(err)
                } else {
                    SessionErrorKind::Unexpected(Box::new(error))
                };
                let muxer_broken = PeerManagerEvent::SessionFailed {
                    sid: session_context.id,
                    kind,
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
                let new_peer_session = PeerManagerEvent::NewSession { pid, pubkey, ctx: session_context };

                self.report_peer(new_peer_session);
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
