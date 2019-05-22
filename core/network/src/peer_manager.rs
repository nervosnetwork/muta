use std::default::Default;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::compat::Stream01CompatExt;
use futures::future::{ready, FutureObj};
use futures::prelude::{FutureExt, Stream, StreamExt, TryFutureExt};
use futures::task::{AtomicWaker, Context, Poll};
use log::{debug, error};
use parking_lot::RwLock;
use rand::seq::SliceRandom;
use std::collections::HashSet;
use tentacle::{multiaddr::Multiaddr, service::DialProtocol};
use tokio::timer::Interval;

use crate::p2p::Dialer;

pub mod discovery;

pub enum Source {
    BootStrap,
    Connected,
    Pool,
}

pub trait PeerManager {
    fn set_disconnected(&mut self, addr: &Multiaddr);

    fn set_connected(&mut self, addr: &Multiaddr);

    /// Return number of connected addresses
    fn connected_count(&self) -> usize;

    fn is_connected(&self, addr: &Multiaddr) -> bool;

    /// Return number of addresses in pool
    fn pool_count(&self) -> usize;

    /// Return given number of addresses
    // FIXME: Change back to Vec<&Multiaddr>, block by discovery protocol,
    // which require 'static. We have Pin now.
    fn addrs(&self, from: Source) -> Vec<Multiaddr>;

    /// Add addresses
    fn add_addrs(&mut self, addrs: Vec<Multiaddr>);

    /// Remove addresses
    fn remove_addrs(&mut self, addrs: Vec<&Multiaddr>);
}

// TODO: remove RwLock
// TODO: add Context to report fatal error
pub struct DefaultPeerManager {
    dialer:          Option<Dialer>,
    waker:           Arc<AtomicWaker>,
    max_connections: usize,

    connected: Arc<RwLock<HashSet<Multiaddr>>>,
    bootstrap: Arc<RwLock<HashSet<Multiaddr>>>,
    pool:      Arc<RwLock<HashSet<Multiaddr>>>,
}

impl DefaultPeerManager {
    pub fn new(max_connections: usize) -> Self {
        DefaultPeerManager {
            dialer: None,
            waker: Arc::new(AtomicWaker::new()),
            max_connections,

            connected: Default::default(),
            bootstrap: Default::default(),
            pool: Default::default(),
        }
    }

    pub fn run(mut self, dialer: Dialer, routine_interval: u64) -> FutureObj<'static, ()> {
        let waker = Arc::clone(&self.waker);
        self.dialer = Some(dialer);

        let job = async move {
            let routine_job = Interval::new_interval(Duration::from_secs(routine_interval))
                .compat()
                .for_each(move |_| {
                    waker.wake();

                    ready(())
                });

            tokio::spawn(routine_job.unit_error().boxed().compat());
            self.for_each(async move |_| ()).await;
        };

        FutureObj::new(Box::new(job))
    }

    fn dial_all(&mut self, addrs: Vec<Multiaddr>) {
        if let Some(dialer) = self.dialer.clone() {
            for addr in addrs.into_iter() {
                match dialer.dial(addr.clone(), DialProtocol::All) {
                    Ok(_) => self.set_connected(&addr),
                    Err(err) => {
                        // FIXME: should retry?
                        debug!("net [p2p]: dial [addr: {}, err: {:?}]", addr, err);
                        self.remove_addrs(vec![&addr]);
                    }
                }
            }
        }
    }

    fn random_unconnected_addrsses(&self, count: usize) -> Vec<Multiaddr> {
        let connected = self.connected.read();
        let bootstrap = self.bootstrap.read();
        let pool = self.pool.read();

        // check bootstrap peer first then pool
        let mut unconnected = bootstrap.difference(&connected).collect::<Vec<_>>();
        let mut pool = pool.difference(&connected).collect::<Vec<_>>();

        // reserve places for bootstrap addresses
        let remain_count = count - unconnected.len();
        let mut rng = rand::thread_rng();
        pool.shuffle(&mut rng);

        unconnected.extend(pool.iter().take(remain_count));

        unconnected
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    }
}

impl Clone for DefaultPeerManager {
    fn clone(&self) -> Self {
        DefaultPeerManager {
            dialer:          self.dialer.clone(),
            waker:           Arc::clone(&self.waker),
            max_connections: self.max_connections,

            connected: Arc::clone(&self.connected),
            bootstrap: Arc::clone(&self.bootstrap),
            pool:      Arc::clone(&self.pool),
        }
    }
}

impl Stream for DefaultPeerManager {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.waker.register(ctx.waker());

        if self.dialer.is_none() {
            // Stop peer manager
            // TODO: maybe report error, restart p2p? Actually, this error
            // should not happen.
            error!("net [peer manager]: fatal error: no dialer found");
            return Poll::Ready(None);
        }

        // Routine connection check
        let connected_count = self.connected.read().len();
        if connected_count > self.max_connections {
            return Poll::Pending;
        }

        let remain_count = self.max_connections - connected_count;
        let unconnected_addresses = self.random_unconnected_addrsses(remain_count);
        // No more addresses to dial
        if unconnected_addresses.is_empty() {
            return Poll::Pending;
        }

        self.dial_all(unconnected_addresses);
        Poll::Ready(Some(()))
    }
}

macro_rules! to_vec {
    ($set:expr) => {
        $set.read()
            .iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    };
}

impl PeerManager for DefaultPeerManager {
    fn set_disconnected(&mut self, addr: &Multiaddr) {
        self.connected.write().remove(addr);
        self.waker.wake();
    }

    fn set_connected(&mut self, addr: &Multiaddr) {
        self.connected.write().insert(addr.clone());
    }

    fn connected_count(&self) -> usize {
        self.connected.read().len()
    }

    fn is_connected(&self, addr: &Multiaddr) -> bool {
        self.connected.read().contains(addr)
    }

    fn pool_count(&self) -> usize {
        self.pool.read().len()
    }

    fn addrs(&self, from: Source) -> Vec<Multiaddr> {
        match from {
            Source::Pool => to_vec!(self.pool),
            Source::Connected => to_vec!(self.connected),
            Source::BootStrap => to_vec!(self.bootstrap),
        }
    }

    // TODO: flush to DB
    fn add_addrs(&mut self, addrs: Vec<Multiaddr>) {
        self.pool.write().extend(addrs);
        self.waker.wake();
    }

    fn remove_addrs(&mut self, addrs: Vec<&Multiaddr>) {
        let mut connected = self.connected.write();
        let mut pool = self.pool.write();

        for addr in addrs.iter() {
            connected.remove(addr);
            pool.remove(addr);
        }

        self.waker.wake();
    }
}
