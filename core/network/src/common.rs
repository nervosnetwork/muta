use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures::{pin_mut, task::AtomicWaker};
use futures_timer::Delay;
use tentacle::multiaddr::{Multiaddr, Protocol};

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
    multi_addr.push(Protocol::Tcp(socket_addr.port()));

    multi_addr
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

            crate::loop_ready!(delay.poll(ctx))?;

            ecg.delay.reset(interval);
            ecg.waker.wake();
        }

        Poll::Pending
    }
}
