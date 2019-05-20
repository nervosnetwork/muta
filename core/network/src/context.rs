use common_channel::Sender;

use crate::error::Error;

#[derive(Clone)]
pub struct Context {
    pub err_tx: Sender<Error>,
}

impl Context {
    pub fn new(err_tx: Sender<Error>) -> Self {
        Context { err_tx }
    }
}
