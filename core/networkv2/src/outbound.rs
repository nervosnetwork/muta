use std::marker::{Send, Sync};
use std::sync::Arc;

use log::error;

use core_networkv2_message::{Codec, Message, Method};

use crate::p2p::{Bytes, Outbound, Scope};
use crate::{CallbackMap, Error};

#[macro_use]
mod r#macro;
mod consensus;
mod sync;
mod tx_pool;

#[derive(Clone)]
pub struct OutboundHandle {
    callback: Arc<CallbackMap>,
    outbound: Arc<Outbound>,
}

impl OutboundHandle {
    pub fn new(callback: Arc<CallbackMap>, outbound: Outbound) -> Self {
        let outbound = Arc::new(outbound);

        OutboundHandle { callback, outbound }
    }
}

unsafe impl Sync for OutboundHandle {}
unsafe impl Send for OutboundHandle {}

pub enum Mode {
    Normal,
    Quick,
}

pub trait BytesBroadcaster<T>
where
    T: Codec,
    Error: From<<T as Codec>::Error>,
{
    fn encode(method: Method, data: T) -> Result<Bytes, Error> {
        let body = data.encode()?;

        let msg = Message {
            method:    method.to_u32(),
            data_size: body.len() as u64,
            data:      body.to_vec(),
        };

        let bytes = msg.encode()?;

        Ok(bytes)
    }

    fn filter_broadcast(&self, method: Method, data: T, scope: Scope) -> Result<(), Error>;
    fn quick_filter_broadcast(&self, method: Method, data: T, scope: Scope) -> Result<(), Error>;

    /// note: simply log error and drop data
    fn silent_broadcast(&self, method: Method, data: T, mode: Mode) {
        if let Err(err) = match mode {
            Mode::Normal => self.filter_broadcast(method, data, Scope::All),
            Mode::Quick => self.quick_filter_broadcast(method, data, Scope::All),
        } {
            error!("net [outbound]: {:?}: [err: {:?}]", method, err);
        }
    }
}

impl<T> BytesBroadcaster<T> for OutboundHandle
where
    T: Codec,
    Error: From<<T as Codec>::Error>,
{
    fn filter_broadcast(&self, method: Method, data: T, scope: Scope) -> Result<(), Error> {
        let bytes = <Self as BytesBroadcaster<T>>::encode(method, data)?;

        self.outbound.filter_broadcast(scope, bytes)?;
        Ok(())
    }

    fn quick_filter_broadcast(&self, method: Method, data: T, scope: Scope) -> Result<(), Error> {
        let bytes = <Self as BytesBroadcaster<T>>::encode(method, data)?;

        self.outbound.quick_filter_broadcast(scope, bytes)?;
        Ok(())
    }
}
