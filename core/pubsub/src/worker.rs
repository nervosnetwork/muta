use std::future::Future;

use futures::channel::oneshot::Sender;
use futures::future::FutureObj;
use runtime::task::JoinHandle;

type TaskFut = FutureObj<'static, ()>;

#[derive(Debug)]
pub struct Control {
    shutdown_tx: Sender<()>,
}

impl Control {
    fn new(shutdown_tx: Sender<()>) -> Self {
        Control { shutdown_tx }
    }

    pub fn shutdown(self) -> Result<(), ()> {
        self.shutdown_tx.send(())
    }
}

pub enum Task {
    Idle(TaskFut),
    Running(JoinHandle<<TaskFut as Future>::Output>),
}

pub struct Worker {
    task:        Task,
    shutdown_tx: Sender<()>,
}

impl Worker {
    pub fn new<Fut>(task: Fut, shutdown_tx: Sender<()>) -> Self
    where
        Fut: Future<Output = ()> + Send + 'static + Unpin,
    {
        let task = Task::Idle(FutureObj::new(Box::new(task)));

        Worker { task, shutdown_tx }
    }

    /// Start task in single thread
    pub fn start_loop(self) -> Self {
        let shutdown_tx = self.shutdown_tx;

        let task = {
            if let Task::Idle(task) = self.task {
                Task::Running(runtime::spawn(task))
            } else {
                self.task
            }
        };

        Worker { task, shutdown_tx }
    }

    /// Return inner idle task fut and shutdown control
    ///
    /// # Panic
    ///
    /// Panics if task is already running
    pub fn task(self) -> (TaskFut, Control) {
        let fut = match self.task {
            Task::Running(_) => panic!("task is running"),
            Task::Idle(fut) => fut,
        };

        let ctrl = Control::new(self.shutdown_tx);

        (fut, ctrl)
    }

    pub async fn shutdown(self) -> Result<(), ()> {
        if let Task::Running(thread_handle) = self.task {
            self.shutdown_tx.send(())?;
            thread_handle.await;
        }

        Ok(())
    }
}
