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
    idx:          Index,
    ident_status: Arc<Mutex<IdentificationStatus>>,
}

impl WaitIdentification {
    fn new(ident_status: Arc<Mutex<IdentificationStatus>>) -> Self {
        WaitIdentification {
            idx: usize::MAX,
            ident_status,
        }
    }
}

impl Future for WaitIdentification {
    type Output = Result<(), super::protocol::Error>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let insert_idx = {
            let idx = self.idx;
            match &mut *self.ident_status.lock() {
                IdentificationStatus::Done(ret) => return Poll::Ready(ret.to_owned()),
                IdentificationStatus::Pending(_) if idx != usize::MAX => return Poll::Pending,
                IdentificationStatus::Pending(wakerset) => wakerset.insert(ctx.waker().to_owned()),
            }
        };

        self.idx = insert_idx;
        Poll::Pending
    }
}

impl Drop for WaitIdentification {
    fn drop(&mut self) {
        if let IdentificationStatus::Pending(wakerset) = &mut *self.ident_status.lock() {
            wakerset.remove(self.idx);
        }
    }
}

pub struct Identification {
    status: Arc<Mutex<IdentificationStatus>>,
}

impl Identification {
    pub(crate) fn new() -> Self {
        Identification {
            status: Default::default(),
        }
    }

    pub fn wait(&self) -> WaitIdentification {
        WaitIdentification::new(Arc::clone(&self.status))
    }

    pub fn pass(&self) {
        self.done(Ok(()))
    }

    pub fn failed(&self, error: super::protocol::Error) {
        self.done(Err(error))
    }

    fn fail_if_not_done(&self) {
        {
            let status = self.status.lock();
            if let IdentificationStatus::Done(_) = &*status {
                return;
            }
        }

        self.failed(super::protocol::Error::WaitFutDropped)
    }

    fn done(&self, ret: Result<(), super::protocol::Error>) {
        let wakerset = {
            let mut status = self.status.lock();

            if let IdentificationStatus::Pending(wakerset) =
                std::mem::replace(&mut *status, IdentificationStatus::Done(ret))
            {
                wakerset
            } else {
                return;
            }
        };

        wakerset.wake()
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
    Done(Result<(), super::protocol::Error>),
}

impl Default for IdentificationStatus {
    fn default() -> Self {
        IdentificationStatus::Pending(WakerSet::new())
    }
}
