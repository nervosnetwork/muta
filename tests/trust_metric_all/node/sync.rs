use core_network::{DiagnosticEvent, TrustReport};
use derive_more::Display;
use protocol::traits::TrustFeedback;
use tokio::sync::{
    broadcast::{channel, Receiver, RecvError, Sender},
    Barrier, BarrierWaitResult, Mutex,
};
use tokio::time::timeout;

use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::Duration,
};

const SYNC_RECV_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Display)]
pub enum SyncError {
    #[display(fmt = "timeout")]
    Timeout,
    #[display(fmt = "recv {}", _0)]
    Recv(RecvError),
    #[display(fmt = "disconnected")]
    Disconected,
}

#[derive(Debug, Display)]
pub enum SyncEvent {
    #[display(fmt = "connected")]
    Connected,
    #[display(fmt = "remote height {}", _0)]
    RemoteHeight(u64),
    #[display(fmt = "feedback {}", _0)]
    TrustMetric(TrustFeedback),
    #[display(fmt = "report {}", _0)]
    TrustReport(TrustReport),
}

#[derive(Clone)]
pub struct Sync {
    diag_tx:   Sender<DiagnosticEvent>,
    diag_rx:   Arc<Mutex<Receiver<DiagnosticEvent>>>,
    barrier:   Arc<Barrier>,
    connected: Arc<AtomicBool>,
}

impl Sync {
    pub fn new() -> Self {
        let (diag_tx, diag_rx) = channel(10);
        let barrier = Arc::new(Barrier::new(2));
        let connected = Arc::new(AtomicBool::new(false));
        let diag_rx = Arc::new(Mutex::new(diag_rx));

        Sync {
            diag_tx,
            diag_rx,
            barrier,
            connected,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn set_connected(&self) {
        self.connected.store(true, Ordering::SeqCst);
    }

    pub fn disconnect(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }

    pub async fn wait(&self) -> BarrierWaitResult {
        self.barrier.wait().await
    }

    // # Panic
    pub async fn wait_connected(&self) {
        let mut count: usize = 2; // Wait client node and full node both be connected to each other
        while count > 0 {
            match self.recv().await {
                Ok(SyncEvent::Connected) => count -= 1,
                Ok(event) => panic!("wait connected, but receive {}", event),
                Err(err) => panic!("connect to full node failed {:?}", err),
            }
        }
        self.set_connected();

        loop {
            match self.recv().await {
                Ok(SyncEvent::RemoteHeight(height)) if height > 0 => break,
                Ok(event) => panic!("wait remote height, but receive {}", event),
                Err(err) => panic!("wait remote height failed {:?}", err),
            }
        }
    }

    pub fn emit(&self, event: DiagnosticEvent) {
        self.diag_tx.send(event).unwrap();
    }

    pub async fn recv(&self) -> Result<SyncEvent, SyncError> {
        match timeout(SYNC_RECV_TIMEOUT, self.diag_rx.lock().await.recv()).await {
            Err(_) if !self.is_connected() => Err(SyncError::Disconected),
            Err(_) => Err(SyncError::Timeout),
            Ok(Err(e)) => Err(SyncError::Recv(e)),
            Ok(Ok(event)) => match event {
                DiagnosticEvent::SessionClosed => {
                    self.disconnect();
                    Err(SyncError::Disconected)
                }
                DiagnosticEvent::RemoteHeight { height } => Ok(SyncEvent::RemoteHeight(height)),
                DiagnosticEvent::TrustMetric { feedback } => Ok(SyncEvent::TrustMetric(feedback)),
                DiagnosticEvent::TrustNewInterval { report } => Ok(SyncEvent::TrustReport(report)),
                DiagnosticEvent::NewSession => Ok(SyncEvent::Connected),
            },
        }
    }
}

impl Drop for Sync {
    fn drop(&mut self) {
        self.connected.store(false, Ordering::SeqCst);
    }
}
