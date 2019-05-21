#![feature(async_await, await_macro, futures_api)]

use std::clone::Clone;
use std::sync::Arc;
use std::{marker::Unpin, pin::Pin};

use futures::prelude::{Sink, Stream};
use futures::task::{AtomicWaker, Context, Poll};

pub use crossbeam_channel::{RecvError, TryRecvError, TrySendError};

pub struct Sender<T> {
    inner: crossbeam_channel::Sender<T>,
    waker: Arc<AtomicWaker>,
}

impl<T> Sender<T> {
    pub fn new(tx: crossbeam_channel::Sender<T>, waker: Arc<AtomicWaker>) -> Self {
        Sender { inner: tx, waker }
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender {
            inner: self.inner.clone(),
            waker: Arc::clone(&self.waker),
        }
    }
}

impl<T> Unpin for Sender<T> {}

impl<T> Sink<T> for Sender<T> {
    type SinkError = TrySendError<T>;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::SinkError> {
        self.try_send(item)?;
        self.waker.wake();

        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }
}

impl<T> Sender<T> {
    pub fn try_send(&self, item: T) -> Result<(), TrySendError<T>> {
        self.inner.try_send(item)?;
        self.waker.wake();

        Ok(())
    }
}

pub struct Receiver<T> {
    inner: crossbeam_channel::Receiver<T>,
    waker: Arc<AtomicWaker>,
}

impl<T> Receiver<T> {
    pub fn new(rx: crossbeam_channel::Receiver<T>, waker: Arc<AtomicWaker>) -> Self {
        Receiver { inner: rx, waker }
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Receiver {
            inner: self.inner.clone(),
            waker: Arc::clone(&self.waker),
        }
    }
}

impl<T> Unpin for Receiver<T> {}

impl<T> Stream for Receiver<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.waker.register(ctx.waker());

        match self.inner.try_recv() {
            Ok(item) => Poll::Ready(Some(item)),
            Err(TryRecvError::Disconnected) => Poll::Ready(None),
            Err(TryRecvError::Empty) => Poll::Pending,
        }
    }
}

impl<T> Receiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.inner.try_recv()
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        self.inner.recv()
    }
}

pub fn bounded<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = crossbeam_channel::bounded(cap);

    wakeable_channel(tx, rx)
}

pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = crossbeam_channel::unbounded();

    wakeable_channel(tx, rx)
}

fn wakeable_channel<T>(
    tx: crossbeam_channel::Sender<T>,
    rx: crossbeam_channel::Receiver<T>,
) -> (Sender<T>, Receiver<T>) {
    let waker = Arc::new(AtomicWaker::new());

    let tx = Sender::new(tx, Arc::clone(&waker));
    let rx = Receiver::new(rx, waker);

    (tx, rx)
}
