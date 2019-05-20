use std::any::Any;
use std::clone::Clone;
use std::marker::{Send, Sync};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use hashbrown::HashMap;
use parking_lot::RwLock;

type CallMap = HashMap<u64, Arc<Box<dyn Any + 'static>>>;

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

    pub fn insert<T: 'static>(&self, uid: u64, value: T) {
        let boxed = Arc::new(Box::new(value) as Box<dyn Any + 'static>);

        self.call_map.write().insert(uid, boxed);
    }

    pub fn take<T: Clone + 'static>(&self, uid: u64) -> Option<T> {
        if let Some(arc_boxed) = self.call_map.write().remove(&uid) {
            arc_boxed.downcast_ref::<T>().map(ToOwned::to_owned)
        } else {
            None
        }
    }
}

unsafe impl Send for CallbackMap {}
unsafe impl Sync for CallbackMap {}
