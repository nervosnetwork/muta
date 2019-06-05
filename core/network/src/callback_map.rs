use std::any::Any;
use std::collections::HashMap;
use std::marker::{Send, Sync};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;

use common_channel::{bounded, Receiver, Sender};
use core_context::Cloneable;

use crate::Error;

pub type SessionId = u64;

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct CallId(u64);

impl CallId {
    pub fn new(v: u64) -> Self {
        CallId(v)
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

impl Cloneable for CallId {}

#[derive(Debug, PartialEq, Eq, Hash)]
struct ChanId {
    call_id: CallId,
    sess_id: SessionId,
}

struct BackChannel(Box<dyn Any + 'static>); // Sender wrapped in boxed Any

// TODO: implement cleanup
#[derive(Default)]
pub struct Callback {
    latest_call_id: AtomicU64,
    cb_chans:       Arc<RwLock<HashMap<ChanId, BackChannel>>>,
}

impl Callback {
    pub fn new() -> Self {
        Callback {
            latest_call_id: AtomicU64::new(0),
            cb_chans:       Default::default(),
        }
    }

    pub fn new_call_id(&self) -> CallId {
        CallId::new(self.latest_call_id.fetch_add(1, Ordering::SeqCst))
    }

    pub fn insert<T: 'static>(&self, call_id: u64, sess_id: usize) -> Receiver<T> {
        let sess_id = sess_id as u64;
        let call_id = CallId::new(call_id);
        let chan_id = ChanId { call_id, sess_id };

        let (done_tx, done_rx) = bounded(1);
        let bchan = BackChannel(Box::new(done_tx));

        self.cb_chans.write().insert(chan_id, bchan);
        done_rx
    }

    pub fn take<T: 'static>(&self, call_id: u64, sess_id: usize) -> Result<Sender<T>, Error> {
        let sess_id = sess_id as u64;
        let call_id = CallId::new(call_id);
        let chan_id = ChanId { call_id, sess_id };

        let BackChannel(boxed_any) = {
            let opt_bchan = self.cb_chans.write().remove(&chan_id);
            opt_bchan.ok_or_else(|| Error::CallbackItemNotFound(call_id.value()))?
        };

        let boxed_chan = boxed_any
            .downcast::<Sender<T>>()
            .map_err(|_| Error::CallbackItemWrongType(call_id.value()))?;

        Ok(*boxed_chan)
    }
}

unsafe impl Send for Callback {}
unsafe impl Sync for Callback {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use crate::Error;

    use super::Callback;

    #[test]
    fn test_new_call_id() {
        let callback = Callback::new();
        assert_eq!(callback.latest_call_id.load(Ordering::SeqCst), 0);

        let call_id = callback.new_call_id();
        assert_eq!(call_id.value(), 0);
        assert_eq!(callback.latest_call_id.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_reach_max_call_id() {
        let callback = Callback::new();
        let max_u64 = u64::max_value();

        callback.latest_call_id.store(max_u64, Ordering::SeqCst);
        assert_eq!(callback.new_call_id().value(), max_u64);
        assert_eq!(callback.new_call_id().value(), 0);
    }

    #[test]
    fn test_insert_then_take() {
        let callback = Callback::new();
        let call_id = callback.new_call_id().value();
        let session_id = 1usize;

        let done_rx = callback.insert::<usize>(call_id, session_id);
        let done_tx = callback.take::<usize>(call_id, session_id).unwrap();

        done_tx.try_send(2077).unwrap();
        assert_eq!(done_rx.try_recv(), Ok(2077));
    }

    #[test]
    fn test_take_wrong_id() {
        let callback = Callback::new();
        let call_id = callback.new_call_id().value();
        let session_id = 1usize;

        callback.insert::<usize>(call_id, session_id);

        match callback.take::<usize>(call_id + 1, session_id) {
            Err(Error::CallbackItemNotFound(id)) => assert_eq!(id, call_id + 1),
            _ => panic!("should return Error::CallbackItemNotFound"),
        }

        match callback.take::<usize>(call_id, session_id + 1) {
            Err(Error::CallbackItemNotFound(id)) => assert_eq!(id, call_id),
            _ => panic!("should return Error::CallbackItemNotFound"),
        }
    }

    #[test]
    fn test_take_wrong_type() {
        let callback = Callback::new();
        let call_id = callback.new_call_id().value();
        let sess_id = 1usize;

        callback.insert::<usize>(call_id, sess_id);
        match callback.take::<u64>(call_id, sess_id) {
            Err(Error::CallbackItemWrongType(id)) => assert_eq!(id, call_id),
            _ => panic!("should return Error::CallbackItemWrongType"),
        }
    }
}
