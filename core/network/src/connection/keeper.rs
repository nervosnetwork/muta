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
    event::{PeerManagerEvent, RemoveKind, RetryKind},
};

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

    fn process_connect_error(&self, error: TentacleError, addr: Multiaddr) {
        use std::io;

        match error {
            TentacleError::ConnectSelf => {
                let connect_self = PeerManagerEvent::AddListenAddr { addr };

                self.report_peer(connect_self);
            }
            TentacleError::RepeatedConnection(sid) => {
                let repeated_connect = PeerManagerEvent::AddSessionAddr { sid, addr };

                self.report_peer(repeated_connect);
            }
            TentacleError::IoError(ref err)
                if err.kind() == io::ErrorKind::TimedOut
                    || err.kind() == io::ErrorKind::Interrupted =>
            {
                let kind = if err.kind() == io::ErrorKind::TimedOut {
                    RetryKind::TimedOut
                } else {
                    RetryKind::Interrupted
                };
                let retry_connect_later = PeerManagerEvent::RetryAddrLater { addr, kind };

                self.report_peer(retry_connect_later);
            }
            _ => {
                let err = Box::new(error);
                let addre = addr.clone();

                let kind = RemoveKind::UnableToConnect { addr: addre, err };
                let unable_to_connect = PeerManagerEvent::RemoveAddr { addr, kind };

                self.report_peer(unable_to_connect);
            }
        }
    }
}

#[rustfmt::skip]
impl ServiceHandle for ConnectionServiceKeeper {
    fn handle_error(&mut self, _ctx: &mut ServiceContext, err: ServiceError) {
        match err {
            ServiceError::DialerError { error, address }
            | ServiceError::ListenError { error, address } => {
                self.process_connect_error(error, address)
            }

            ServiceError::ProtocolSelectError { session_context, proto_name } => {
                let pid = peer_pubkey!(&session_context).peer_id();

                let report = if let Some(proto_name) = proto_name {
                    let kind = RemoveKind::UnknownProtocol(proto_name);

                    PeerManagerEvent::RemovePeer { pid, kind }
                } else {
                    // maybe unstable connection
                    let kind = RetryKind::ProtocolSelect;

                    PeerManagerEvent::RetryPeerLater { pid, kind }
                };

                self.report_peer(report);
            }

            ServiceError::ProtocolError { id, error, proto_id } => {
                let kind = RemoveKind::BrokenProtocol {
                    proto_id,
                    err: Box::new(error),
                };
                let broken_protocol = PeerManagerEvent::RemovePeerBySession { sid: id, kind };

                self.report_peer(broken_protocol);
            }

            ServiceError::SessionTimeout { session_context } => {
                let pid = peer_pubkey!(&session_context).peer_id();

                let kind = RetryKind::TimedOut;
                let retry_peer_later = PeerManagerEvent::RetryPeerLater { pid, kind };

                self.report_peer(retry_peer_later);
            }

            ServiceError::MuxerError { session_context, error } => {
                let pid = peer_pubkey!(&session_context).peer_id();

                let kind = RetryKind::Multiplex(Box::new(error));
                let retry_peer_later = PeerManagerEvent::RetryPeerLater { pid, kind };

                self.report_peer(retry_peer_later);
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
                let pid = peer_pubkey!(&session_context).peer_id();
                let sid = session_context.id;

                let kind = RemoveKind::SessionBlocked { pid, sid };
                let blocked_session = PeerManagerEvent::RemovePeerBySession { sid, kind };

                self.report_peer(blocked_session);
                self.report_error(ErrorKind::SessionBlocked);
            }
        }
    }

    fn handle_event(&mut self, _ctx: &mut ServiceContext, evt: ServiceEvent) {
        match evt {
            ServiceEvent::SessionOpen { session_context } => {
                let pubkey = peer_pubkey!(&session_context).clone();
                let pid = pubkey.peer_id();
                let pidd = pid.clone();
                let addr = session_context.address.clone();
                let sid = Some(session_context.id);

                let add_peer_addr = PeerManagerEvent::AddPeer { pid: pidd, pubkey, addr };
                let attach_session = PeerManagerEvent::UpdatePeerSession { pid, sid };

                self.report_peer(add_peer_addr);
                self.report_peer(attach_session);
            }
            ServiceEvent::SessionClose { session_context } => {
                let pid = peer_pubkey!(&session_context).peer_id();
                let sid = None;

                let detach_session = PeerManagerEvent::UpdatePeerSession { pid, sid };

                self.report_peer(detach_session);
            }
            ServiceEvent::ListenStarted { address } => {
                let start_listen = PeerManagerEvent::AddListenAddr { addr: address };

                self.report_peer(start_listen);
            }
            ServiceEvent::ListenClose { address } => {
                let close_listen = PeerManagerEvent::RemoveListenAddr { addr: address };

                self.report_peer(close_listen);
            }
        }
    }
}
