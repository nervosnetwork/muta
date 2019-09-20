use std::{io, marker::PhantomData};

use async_trait::async_trait;
use futures::channel::{mpsc::UnboundedSender, oneshot};
use log::debug;
use protocol::{traits::Priority, types::UserAddress};
use tentacle::{
    bytes::Bytes,
    error::Error as TentacleError,
    service::{ServiceControl, TargetSession},
};

use crate::{
    error::NetworkError,
    event::{MultiUsersMessage, PeerManagerEvent},
    traits::{MessageSender, NetworkProtocol},
};

pub struct ConnectionServiceControl<P: NetworkProtocol> {
    inner:  ServiceControl,
    mgr_tx: UnboundedSender<PeerManagerEvent>,

    // Indicate which protocol this connection service control
    pin_protocol: PhantomData<fn() -> P>,
}

impl<P: NetworkProtocol> ConnectionServiceControl<P> {
    pub fn new(control: ServiceControl, mgr_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        ConnectionServiceControl {
            inner: control,
            mgr_tx,

            pin_protocol: PhantomData,
        }
    }
}

impl<P: NetworkProtocol> Clone for ConnectionServiceControl<P> {
    fn clone(&self) -> Self {
        ConnectionServiceControl {
            inner:  self.inner.clone(),
            mgr_tx: self.mgr_tx.clone(),

            pin_protocol: PhantomData,
        }
    }
}

#[async_trait]
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

    async fn users_send(
        &self,
        user_addrs: Vec<UserAddress>,
        msg: Bytes,
        pri: Priority,
    ) -> Result<(), NetworkError> {
        let (miss_tx, miss_rx) = oneshot::channel();
        let users_msg = MultiUsersMessage {
            user_addrs,
            msg,
            pri,
        };
        let route_users_msg = PeerManagerEvent::RouteMultiUsersMessage { users_msg, miss_tx };

        if self.mgr_tx.unbounded_send(route_users_msg).is_err() {
            debug!("network: connection service control: peer manager service exit");
        }

        let missed_users = miss_rx
            .await
            .map_err(|err| NetworkError::UnexpectedError(Box::new(err)))?;

        if !missed_users.is_empty() {
            Err(NetworkError::PartialRouteMessage { miss: missed_users })
        } else {
            Ok(())
        }
    }
}
