use std::borrow::Borrow;
use std::collections::HashSet;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use parking_lot::Mutex;

type Index = usize;

pub struct WaitIdentification {
    idx:   Index,
    ident: Identification,
}

impl WaitIdentification {
    pub fn new(ident: Identification) -> Self {
        WaitIdentification {
            idx: usize::MAX,
            ident,
        }
    }
}

impl Future for WaitIdentification {
    type Output = Result<(), ()>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let updated_idx = {
            let cur_idx = self.idx;

            match &mut *self.ident.status.lock() {
                IdentificationStatus::Done(ret) => return Poll::Ready(ret.to_owned()),
                IdentificationStatus::Pending(_) if cur_idx != usize::MAX => return Poll::Pending,
                IdentificationStatus::Pending(wakerset) => wakerset.insert(ctx.waker().to_owned()),
            }
        };

        self.idx = updated_idx;
        Poll::Pending
    }
}

impl Drop for WaitIdentification {
    fn drop(&mut self) {
        match &mut *self.ident.status.lock() {
            IdentificationStatus::Pending(wakerset) => wakerset.remove(self.idx),
            _ => (),
        }
    }
}

#[derive(Clone)]
pub struct Identification {
    status: Arc<Mutex<IdentificationStatus>>,
}

impl Identification {
    pub fn new() -> Self {
        Identification {
            status: Default::default(),
        }
    }

    pub fn wait(&self) -> WaitIdentification {
        WaitIdentification::new(self.clone())
    }

    pub fn pass(&self) {
        self.done(Ok(()))
    }

    pub fn failed(&self) {
        self.done(Err(()))
    }

    fn fail_if_not_done(&self) {
        {
            let status = self.status.lock();
            if let IdentificationStatus::Done(_) = &*status {
                return;
            }
        }

        self.failed()
    }

    fn done(&self, ret: Result<(), ()>) {
        let mut status = self.status.lock();

        match std::mem::replace(&mut *status, IdentificationStatus::Done(ret)) {
            IdentificationStatus::Pending(workerset) => workerset.wake(),
            _ => (),
        }
    }
}

impl Drop for Identification {
    fn drop(&mut self) {
        self.fail_if_not_done()
    }
}

struct IndexedWaker {
    idx:   Index,
    waker: Waker,
}

impl IndexedWaker {
    fn wake(self) {
        self.waker.wake()
    }
}

impl Borrow<Index> for IndexedWaker {
    fn borrow(&self) -> &Index {
        &self.idx
    }
}

impl PartialEq for IndexedWaker {
    fn eq(&self, other: &IndexedWaker) -> bool {
        self.idx == other.idx
    }
}

impl Eq for IndexedWaker {}

impl Hash for IndexedWaker {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.idx.hash(state)
    }
}

struct WakerSet {
    id:     Index,
    wakers: HashSet<IndexedWaker>,
}

impl WakerSet {
    fn new() -> WakerSet {
        WakerSet {
            id:     0,
            wakers: HashSet::new(),
        }
    }

    fn insert(&mut self, waker: Waker) -> Index {
        debug_assert!(self.id != std::usize::MAX);
        self.id += 1;

        let indexed_waker = IndexedWaker {
            idx: self.id,
            waker,
        };

        self.wakers.insert(indexed_waker);
        self.id
    }

    fn remove(&mut self, idx: Index) {
        self.wakers.remove(&idx);
    }

    fn wake(self) {
        for waker in self.wakers {
            waker.wake()
        }
    }
}

enum IdentificationStatus {
    Pending(WakerSet),
    Done(Result<(), ()>),
}

impl Default for IdentificationStatus {
    fn default() -> Self {
        IdentificationStatus::Pending(WakerSet::new())
    }
}
