use futures::future::{err, ok, Either};
use futures::prelude::Future;
use futures::sync::oneshot::{channel, Sender};
use log::error;
use tokio;

use std::thread::{self as thread, JoinHandle};

pub type Task = Box<dyn Future<Item = (), Error = ()> + Send + 'static>;

pub struct ServiceWorker {
    shutdown_tx: Sender<()>,
    thread_handle: JoinHandle<()>,
}

impl ServiceWorker {
    pub fn kick_start(worker: Task) -> Self {
        let (shutdown_tx, shutdown_rx) = channel();
        let worker = worker.select2(shutdown_rx).then(|res| -> Task {
            match res {
                Ok(Either::A(_)) => Box::new(ok(())),
                Ok(Either::B(_)) => Box::new(ok(())),
                _ => Box::new(err(())),
            }
        });
        let thread_handle = thread::spawn(move || tokio::run(worker));

        ServiceWorker {
            shutdown_tx,
            thread_handle,
        }
    }

    pub fn shutdown(self) -> Result<(), ()> {
        self.shutdown_tx.send(())?;
        self.thread_handle.join().map_err(|err| {
            error!("Network: worker thread join error: {:?}", err);
        })?;

        Ok(())
    }
}
