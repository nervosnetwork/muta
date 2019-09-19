use std::{
    any::Any,
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
};

use derive_more::Constructor;
use futures::channel::oneshot::{self, Receiver, Sender};
use parking_lot::RwLock;
use tentacle::SessionId;

use crate::error::{ErrorKind, NetworkError};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Constructor)]
struct Key {
    sid: SessionId,
    rid: u64,
}

struct BackSender(Box<Arc<dyn Any + Send + Sync + 'static>>);

#[derive(Default)]
pub struct RpcMap {
    next_id: Arc<AtomicU64>,
    map:     Arc<RwLock<HashMap<Key, BackSender>>>,
}

impl RpcMap {
    pub fn new() -> Self {
        RpcMap {
            next_id: Arc::new(AtomicU64::new(0)),
            map:     Default::default(),
        }
    }

    pub fn next_rpc_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn insert<T: Send + 'static>(&self, sid: SessionId, rid: u64) -> Receiver<T> {
        let key = Key::new(sid, rid);

        let (done_tx, done_rx) = oneshot::channel();
        let sender = BackSender(Box::new(Arc::new(done_tx)));

        self.map.write().insert(key, sender);

        done_rx
    }

    pub fn take<T: Send + 'static>(
        &self,
        sid: SessionId,
        rid: u64,
    ) -> Result<Sender<T>, NetworkError> {
        let key = Key::new(sid, rid);

        if !self.map.read().contains_key(&key) {
            return Err(ErrorKind::UnknownRpc { sid, rid }.into());
        }

        let BackSender(boxed_any) = {
            let opt_sender = self.map.write().remove(&key);
            opt_sender.ok_or_else(|| ErrorKind::UnknownRpc { sid, rid })?
        };

        let arc_sender: Arc<Sender<T>> = boxed_any
            .downcast::<Sender<T>>()
            .map_err(|_| ErrorKind::UnexpectedRpcSender)?;

        Arc::try_unwrap(arc_sender).map_err(|_| ErrorKind::MoreArcRpcSender.into())
    }
}
