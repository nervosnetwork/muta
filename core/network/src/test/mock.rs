use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use protocol::Bytes;
use tentacle::multiaddr::Multiaddr;
use tentacle::secio::{PublicKey, SecioKeyPair};
use tentacle::service::{SessionType, TargetProtocol};
use tentacle::{ProtocolId, SessionId};

#[derive(Clone, Debug)]
pub struct SessionContext {
    pub id:            SessionId,
    pub address:       Multiaddr,
    pub ty:            SessionType,
    pub remote_pubkey: Option<PublicKey>,
    pending_data_size: Arc<AtomicUsize>,
}

impl SessionContext {
    pub fn no_encrypted(id: SessionId, ty: SessionType) -> Self {
        let address = "/ip4/47.111.169.36/tcp/3000".parse().expect("multiaddr");

        SessionContext {
            id,
            address,
            ty,
            remote_pubkey: None,
            pending_data_size: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn random(id: SessionId, ty: SessionType) -> Self {
        let keypair = SecioKeyPair::secp256k1_generated();
        let pubkey = keypair.public_key();
        let peer_id = pubkey.peer_id();

        let address = {
            let addr_str = format!("/ip4/47.111.169.36/tcp/3000/p2p/{}", peer_id.to_base58());
            addr_str.parse().expect("multiaddr")
        };

        SessionContext {
            id,
            address,
            ty,
            remote_pubkey: Some(pubkey),
            pending_data_size: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn make(id: SessionId, address: Multiaddr, ty: SessionType, pubkey: PublicKey) -> Self {
        SessionContext {
            id,
            address,
            ty,
            remote_pubkey: Some(pubkey),
            pending_data_size: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn pending_data_size(&self) -> usize {
        self.pending_data_size.load(Ordering::SeqCst)
    }

    pub fn arced(self) -> Arc<SessionContext> {
        Arc::new(self)
    }
}

impl From<Arc<tentacle::context::SessionContext>> for SessionContext {
    fn from(ctx: Arc<tentacle::context::SessionContext>) -> Self {
        SessionContext {
            id:                ctx.id,
            address:           ctx.address.to_owned(),
            ty:                ctx.ty,
            remote_pubkey:     ctx.remote_pubkey.clone(),
            pending_data_size: Arc::new(AtomicUsize::new(ctx.pending_data_size())),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ControlEvent {
    SendMessage {
        proto_id:   ProtocolId,
        session_id: SessionId,
        msg:        Bytes,
    },
    Disconnect {
        session_id: SessionId,
    },
    OpenProtocols {
        session_id:   SessionId,
        target_proto: TargetProtocol,
    },
}

#[derive(Clone)]
pub struct ServiceControl {
    pub event: Arc<Mutex<Option<ControlEvent>>>,
}

impl Default for ServiceControl {
    fn default() -> Self {
        ServiceControl {
            event: Arc::new(Mutex::new(None)),
        }
    }
}

impl ServiceControl {
    pub fn event(&self) -> Option<ControlEvent> {
        self.event.lock().clone()
    }

    pub fn quick_send_message_to(
        &self,
        session_id: SessionId,
        proto_id: ProtocolId,
        msg: Bytes,
    ) -> Result<(), String> {
        *self.event.lock() = Some(ControlEvent::SendMessage {
            session_id,
            proto_id,
            msg,
        });

        Ok(())
    }

    pub fn disconnect(&self, session_id: SessionId) {
        *self.event.lock() = Some(ControlEvent::Disconnect { session_id });
    }

    pub fn open_protocols(
        &self,
        session_id: SessionId,
        target_proto: TargetProtocol,
    ) -> Result<(), String> {
        *self.event.lock() = Some(ControlEvent::OpenProtocols {
            session_id,
            target_proto,
        });

        Ok(())
    }
}

pub struct ProtocolContext {
    proto_id:    ProtocolId,
    pub session: SessionContext,
    pub control: ServiceControl,
}

impl ProtocolContext {
    pub fn make_no_encrypted(proto_id: ProtocolId, id: SessionId, ty: SessionType) -> Self {
        ProtocolContext {
            proto_id,
            session: SessionContext::no_encrypted(id, ty),
            control: ServiceControl::default(),
        }
    }

    pub fn make(proto_id: ProtocolId, id: SessionId, ty: SessionType) -> Self {
        ProtocolContext {
            proto_id,
            session: SessionContext::random(id, ty),
            control: ServiceControl::default(),
        }
    }

    pub fn proto_id(&self) -> ProtocolId {
        self.proto_id
    }

    pub fn control(&self) -> &ServiceControl {
        &self.control
    }

    pub fn disconnect(&self, session_id: SessionId) {
        self.control.disconnect(session_id)
    }
}
