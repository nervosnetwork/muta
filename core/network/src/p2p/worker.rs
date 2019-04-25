use futures::future::{err, ok, Either};
use futures::prelude::Future;
use futures::sync::oneshot::{channel, Sender};
use futures03::compat::Future01CompatExt;
use runtime::task::{spawn, JoinHandle};

pub type Task = Box<dyn Future<Item = (), Error = ()> + Send + 'static>;

pub struct ServiceWorker {
    shutdown_tx:   Sender<()>,
    thread_handle: JoinHandle<Result<(), ()>>,
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
        let thread_handle = spawn(async move { await!(worker.compat()) });

        ServiceWorker {
            shutdown_tx,
            thread_handle,
        }
    }

    pub async fn shutdown(self) -> Result<(), ()> {
        self.shutdown_tx.send(())?;
        await!(self.thread_handle)?;

        Ok(())
    }
}
