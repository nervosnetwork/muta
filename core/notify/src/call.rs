use futures::sync::{mpsc, oneshot};
use futures::Future;

pub struct Request<A, R> {
    pub responder: oneshot::Sender<R>,
    pub arguments: A,
}

impl<A, R> Request<A, R> {
    pub fn call(sender: &mut mpsc::Sender<Request<A, R>>, arguments: A) -> Option<R> {
        let (responder, response) = oneshot::channel();
        let _ = sender.try_send(Request {
            responder,
            arguments,
        });
        response.wait().ok()
    }
}
