use std::{
    borrow::Cow,
    future::Future,
    net::SocketAddr,
    ops::Add,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use derive_more::Display;
use futures::{pin_mut, task::AtomicWaker};
use futures_timer::Delay;
use serde_derive::{Deserialize, Serialize};
use tentacle::{
    multiaddr::{Multiaddr, Protocol},
    secio::PeerId,
};

use crate::traits::MultiaddrExt;

#[macro_export]
macro_rules! loop_ready {
    ($poll:expr) => {
        match $poll {
            Poll::Pending => break,
            Poll::Ready(v) => v,
        }
    };
}

#[macro_export]
macro_rules! service_ready {
    ($service:expr, $poll:expr) => {
        match crate::loop_ready!($poll) {
            Some(v) => v,
            None => {
                log::info!("network: {} exit", $service);
                return Poll::Ready(());
            }
        }
    };
}

pub fn socket_to_multi_addr(socket_addr: SocketAddr) -> Multiaddr {
    let mut multi_addr = Multiaddr::from(socket_addr.ip());
    multi_addr.push(Protocol::TCP(socket_addr.port()));

    multi_addr
}

impl MultiaddrExt for Multiaddr {
    fn id_bytes(&self) -> Option<Cow<'_, [u8]>> {
        for proto in self.iter() {
            match proto {
                Protocol::P2P(bytes) => return Some(bytes),
                _ => (),
            }
        }

        None
    }

    fn has_id(&self) -> bool {
        self.iter().any(|proto| match proto {
            Protocol::P2P(_) => true,
            _ => false,
        })
    }

    fn push_id(&mut self, peer_id: PeerId) {
        self.push(Protocol::P2P(Cow::Owned(peer_id.as_bytes().to_vec())))
    }
}

pub struct HeartBeat {
    waker:    Arc<AtomicWaker>,
    interval: Duration,
    delay:    Delay,
}

impl HeartBeat {
    pub fn new(waker: Arc<AtomicWaker>, interval: Duration) -> Self {
        let delay = Delay::new(interval);

        HeartBeat {
            waker,
            interval,
            delay,
        }
    }
}

// # Note
//
// Delay returns an error after default global timer gone away.
impl Future for HeartBeat {
    type Output = <Delay as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let ecg = &mut self.as_mut();

        loop {
            let interval = ecg.interval;
            let delay = &mut ecg.delay;
            pin_mut!(delay);

            crate::loop_ready!(delay.poll(ctx));

            let next_time = Instant::now().add(interval);
            ecg.delay.reset(next_time);
            ecg.waker.wake();
        }

        Poll::Pending
    }
}

#[derive(Debug, Display, PartialEq, Eq, Serialize, Deserialize, Clone)]
#[display(fmt = "{}:{}", host, port)]
pub struct ConnectedAddr {
    host: String,
    port: u16,
}

impl From<&Multiaddr> for ConnectedAddr {
    fn from(multiaddr: &Multiaddr) -> Self {
        use tentacle::multiaddr::Protocol::*;

        let mut host = None;
        let mut port = 0u16;

        for comp in multiaddr.iter() {
            match comp {
                IP4(ip_addr) => host = Some(ip_addr.to_string()),
                IP6(ip_addr) => host = Some(ip_addr.to_string()),
                DNS4(dns_addr) | DNS6(dns_addr) => host = Some(dns_addr.to_string()),
                TLS(tls_addr) => host = Some(tls_addr.to_string()),
                TCP(p) => port = p,
                _ => (),
            }
        }

        let host = host.unwrap_or_else(|| multiaddr.to_string());
        ConnectedAddr { host, port }
    }
}
