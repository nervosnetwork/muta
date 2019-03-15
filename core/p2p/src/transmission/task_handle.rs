use futures::task::Task;
use parking_lot::RwLock;

use std::sync::Arc;

pub type TaskId = usize;

/// Recv data task id
pub const RECV_DATA_TASK_ID: TaskId = 1;

// Wrapper around cast and recv stream task
pub(crate) struct TaskHandle {
    inner: Arc<RwLock<Option<Task>>>,
}

impl TaskHandle {
    pub fn notify(&self, id: TaskId) {
        let maybe_task = match id {
            RECV_DATA_TASK_ID => self.inner.read().clone(),
            _ => unreachable!(),
        };

        maybe_task.and_then(|task| {
            task.notify();
            Some(())
        });
    }

    pub fn insert(&mut self, id: TaskId, task: Task) {
        match id {
            RECV_DATA_TASK_ID => *self.inner.write() = Some(task),
            _ => unreachable!(),
        }
    }
}

impl Default for TaskHandle {
    fn default() -> Self {
        TaskHandle {
            inner: Arc::new(RwLock::new(None)),
        }
    }
}

impl Clone for TaskHandle {
    fn clone(&self) -> Self {
        TaskHandle {
            inner: Arc::clone(&self.inner),
        }
    }
}
