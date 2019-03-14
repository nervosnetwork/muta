use futures::task::Task;
use parking_lot::RwLock;

use std::sync::Arc;

pub type TaskId = usize;

/// Broadcast task id
pub const BROADCAST_TASK_ID: TaskId = 1;
/// Recv data task id
pub const RECV_DATA_TASK_ID: TaskId = 2;

// Wrapper around cast and recv stream task
pub(crate) struct TaskHandle {
    inner: Arc<RwLock<(Option<Task>, Option<Task>)>>,
}

impl TaskHandle {
    pub fn notify(&self, id: TaskId) {
        let maybe_task = match id {
            BROADCAST_TASK_ID => self.inner.read().0.clone(),
            RECV_DATA_TASK_ID => self.inner.read().1.clone(),
            _ => unreachable!(),
        };

        maybe_task.and_then(|task| {
            task.notify();
            Some(())
        });
    }

    pub fn insert(&mut self, id: TaskId, task: Task) {
        match id {
            BROADCAST_TASK_ID => self.inner.write().0 = Some(task),
            RECV_DATA_TASK_ID => self.inner.write().1 = Some(task),
            _ => unreachable!(),
        }
    }
}

impl Default for TaskHandle {
    fn default() -> Self {
        TaskHandle {
            inner: Arc::new(RwLock::new((None, None))),
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
