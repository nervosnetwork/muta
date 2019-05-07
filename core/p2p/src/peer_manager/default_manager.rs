use crate::peer_manager::{DefaultPeerManagerImpl, PeerManager};

use tentacle::multiaddr::Multiaddr;

use std::borrow::{Borrow, BorrowMut};
use std::default::Default;

pub trait BorrowExt {
    fn borrow<M>(&self) -> &M
    where
        Self: Borrow<M>;
}

pub trait BorrowMutExt {
    fn borrow_mut<M>(&mut self) -> &mut M
    where
        Self: BorrowMut<M>;
}

pub type DefaultPeerManager = PeerManagerHandle<DefaultPeerManagerImpl>;

impl DefaultPeerManager {
    pub fn new() -> Self {
        PeerManagerHandle {
            inner: DefaultPeerManagerImpl::new(),
        }
    }

    pub fn register_self(&mut self, addrs: Vec<Multiaddr>) {
        self.inner
            .local_listen_addrs
            .write()
            .extend(addrs.into_iter());
    }
}

impl Default for DefaultPeerManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PeerManagerHandle<I> {
    pub(crate) inner: I,
}

impl<M: Clone> Clone for PeerManagerHandle<M> {
    fn clone(&self) -> Self {
        PeerManagerHandle {
            inner: self.inner.clone(),
        }
    }
}

impl<M: PeerManager> Borrow<M> for PeerManagerHandle<M> {
    fn borrow(&self) -> &M {
        &self.inner
    }
}

impl<M: PeerManager> BorrowMut<M> for PeerManagerHandle<M> {
    fn borrow_mut(&mut self) -> &mut M {
        &mut self.inner
    }
}

impl<M> BorrowExt for PeerManagerHandle<M> {
    fn borrow<T>(&self) -> &T
    where
        Self: Borrow<T>,
    {
        <Self as Borrow<T>>::borrow(self)
    }
}
impl<M> BorrowMutExt for PeerManagerHandle<M> {
    fn borrow_mut<T>(&mut self) -> &mut T
    where
        Self: BorrowMut<T>,
    {
        <Self as BorrowMut<T>>::borrow_mut(self)
    }
}
