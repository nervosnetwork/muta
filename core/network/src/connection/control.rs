use std::{io, marker::PhantomData};

use protocol::traits::Priority;
use tentacle::{
    bytes::Bytes,
    error::Error as TentacleError,
    service::{ServiceControl, TargetSession},
};

use crate::{
    error::NetworkError,
    traits::{MessageSender, NetworkProtocol},
};

pub struct ConnectionServiceControl<P: NetworkProtocol> {
    inner: ServiceControl,

    // Indicate which protocol this connection service control
    pin_protocol: PhantomData<fn() -> P>,
}

impl<P: NetworkProtocol> ConnectionServiceControl<P> {
    pub fn new(control: ServiceControl) -> Self {
        ConnectionServiceControl {
            inner: control,

            pin_protocol: PhantomData,
        }
    }
}

impl<P: NetworkProtocol> Clone for ConnectionServiceControl<P> {
    fn clone(&self) -> Self {
        ConnectionServiceControl {
            inner: self.inner.clone(),

            pin_protocol: PhantomData,
        }
    }
}

impl<P: NetworkProtocol> MessageSender for ConnectionServiceControl<P> {
    fn send(&self, tar: TargetSession, msg: Bytes, pri: Priority) -> Result<(), NetworkError> {
        let proto_id = P::message_proto_id();

        let ret = match pri {
            Priority::High => self.inner.quick_filter_broadcast(tar, proto_id, msg),
            Priority::Normal => self.inner.filter_broadcast(tar, proto_id, msg),
        };

        ret.map_err(|err| match &err {
            TentacleError::IoError(io_err) => match io_err.kind() {
                io::ErrorKind::BrokenPipe => NetworkError::Shutdown,
                io::ErrorKind::WouldBlock => NetworkError::Busy,
                _ => NetworkError::UnexpectedError(Box::new(err)),
            },
            _ => NetworkError::UnexpectedError(Box::new(err)),
        })?;

        Ok(())
    }
}
