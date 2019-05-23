use std::any::Any;
use std::clone::Clone;
use std::collections::HashMap;
use std::marker::{Send, Sync};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use log::debug;
use parking_lot::RwLock;

type AnyBox = Box<dyn Any + 'static>;
type CallMap = HashMap<u64, Arc<AnyBox>>;

// TODO: implement cleanup
#[derive(Default)]
pub struct CallbackMap {
    latest_uid: AtomicU64,
    call_map:   Arc<RwLock<CallMap>>,
}

impl CallbackMap {
    pub fn new() -> Self {
        CallbackMap {
            latest_uid: AtomicU64::new(0),
            call_map:   Default::default(),
        }
    }

    pub fn new_uid(&self) -> u64 {
        self.latest_uid.fetch_add(1, Ordering::SeqCst)
    }

    pub fn insert<T: Clone + 'static>(&self, uid: u64, value: T) {
        let boxed_any = Arc::new(Box::new(value) as AnyBox);

        debug!(
            "net [callback]: insert [uid: {}, type_id: {:?}]",
            uid,
            boxed_any.type_id()
        );

        self.call_map.write().insert(uid, boxed_any);
    }

    pub fn take<T: Clone + 'static>(&self, uid: u64) -> Option<T> {
        if let Some(boxed_any) = self.call_map.write().remove(&uid) {
            debug!(
                "net [callback]: take [uid: {}, type_id: {:?}]",
                uid,
                boxed_any.type_id()
            );

            boxed_any.downcast_ref::<T>().map(ToOwned::to_owned)
        } else {
            None
        }
    }
}

unsafe impl Send for CallbackMap {}
unsafe impl Sync for CallbackMap {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use common_channel::{bounded, Sender};

    use super::CallbackMap;

    #[test]
    fn test_new_uid() {
        let callback = CallbackMap::new();

        let uid = callback.new_uid();
        let new_uid = callback.new_uid();
        assert_eq!(uid + 1, new_uid);
    }

    #[test]
    fn test_reach_max_uid() {
        let callback = CallbackMap::new();

        callback
            .latest_uid
            .store(u64::max_value(), Ordering::SeqCst);

        assert_eq!(callback.new_uid(), u64::max_value());
        assert_eq!(callback.new_uid(), 0);
    }

    #[test]
    fn test_insert_then_take() {
        let callback = CallbackMap::new();

        let uid = callback.new_uid();
        let (done_tx, done_rx) = bounded::<usize>(1);

        callback.insert(uid, done_tx);
        let done_tx = callback.take::<Sender<usize>>(uid);
        assert!(done_tx.is_some());

        assert!(done_tx.unwrap().try_send(1).is_ok());
        assert_eq!(done_rx.try_recv(), Ok(1));
    }

    #[test]
    fn test_take_wrong_type() {
        let callback = CallbackMap::new();

        let uid = callback.new_uid();
        let (done_tx, _) = bounded::<usize>(1);

        callback.insert(uid, done_tx);
        let done_tx = callback.take::<Sender<u64>>(uid);
        assert!(done_tx.is_none());
    }
}
