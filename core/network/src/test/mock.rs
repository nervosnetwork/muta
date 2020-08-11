use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tentacle::multiaddr::Multiaddr;
use tentacle::secio::PublicKey;
use tentacle::service::SessionType;
use tentacle::SessionId;

#[derive(Clone, Debug)]
pub struct SessionContext {
    pub id:            SessionId,
    pub address:       Multiaddr,
    pub ty:            SessionType,
    pub remote_pubkey: Option<PublicKey>,
    pending_data_size: Arc<AtomicUsize>,
}

impl SessionContext {
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
