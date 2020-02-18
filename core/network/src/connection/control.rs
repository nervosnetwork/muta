use std::{io, marker::PhantomData};

use async_trait::async_trait;
use futures::channel::{mpsc::UnboundedSender, oneshot};
use log::debug;
use protocol::{traits::Priority, types::Address, Bytes};
use tentacle::{
    error::Error as TentacleError,
    service::{ServiceControl, TargetSession},
    SessionId,
};

use crate::{
    error::NetworkError,
    event::{MultiUsersMessage, PeerManagerEvent},
    traits::{MessageSender, NetworkProtocol, SessionQuerier},
};

pub struct ConnectionServiceControl<P: NetworkProtocol, Q: SessionQuerier> {
    inner:        ServiceControl,
    mgr_tx:       UnboundedSender<PeerManagerEvent>,
    sessions: Q,

    // Indicate which protocol this connection service control
    pin_protocol: PhantomData<fn() -> P>,
}

impl<P: NetworkProtocol, Q: SessionQuerier> ConnectionServiceControl<P, Q> {
    pub fn new(
        control: ServiceControl,
        mgr_tx: UnboundedSender<PeerManagerEvent>,
        session_querier: Q,
    ) -> Self {
        ConnectionServiceControl {
            inner: control,
            mgr_tx,
            sessions: session_querier,

            pin_protocol: PhantomData,
        }
    }

    pub fn filter_blocked(
        &self,
        tar: TargetSession,
    ) -> (Option<TargetSession>, Option<Vec<SessionId>>) {
        self.sessions.refresh_blocked();

        let all_blocked = self.sessions.all_blocked();
        if all_blocked.is_empty() {
            return (Some(tar), None);
        }

        match tar {
            TargetSession::Single(sid) => {
                if all_blocked.contains(&sid) {
                    (None, Some(vec![sid]))
                }else {
                    (Some(TargetSession::Single(sid)), None)
                }
            }
            TargetSession::Multi(sids) => {
                let (sendable, blocked): (Vec<SessionId>, Vec<SessionId>) =
                    sids.into_iter().partition(|sid| !all_blocked.contains(sid));

                if sendable.is_empty() && blocked.is_empty() {
                    unreachable!()
                } else if sendable.is_empty() {
                    (None, Some(blocked))
                } else if blocked.is_empty() {
                    (Some(TargetSession::Multi(sendable)), None)
                } else {
                    (Some(TargetSession::Multi(sendable)), Some(blocked))
                }
            }
            TargetSession::All => {
                let sendable = self.sessions.all_sendable();

                (Some(TargetSession::Multi(sendable)), Some(all_blocked))
            }
        }
    }
}

impl<P: NetworkProtocol, Q: SessionQuerier + Clone> Clone for ConnectionServiceControl<P, Q> {
    fn clone(&self) -> Self {
        ConnectionServiceControl {
            inner:        self.inner.clone(),
            mgr_tx:       self.mgr_tx.clone(),
            sessions: self.sessions.clone(),

            pin_protocol: PhantomData,
        }
    }
}

#[async_trait]
impl<P, Q> MessageSender for ConnectionServiceControl<P, Q>
where
    P: NetworkProtocol,
    Q: SessionQuerier + Send + Sync + Unpin + 'static,
{
    fn send(&self, tar: TargetSession, msg: Bytes, pri: Priority) -> Result<(), NetworkError> {
        let proto_id = P::message_proto_id();

        let (tar, opt_blocked) = match self.filter_blocked(tar) {
            (None, None) => unreachable!(),
            (None, blocked) => {
                return Err(NetworkError::Send {
                    blocked,
                    other: None,
                })
            }
            (Some(tar), opt_blocked) => (tar, opt_blocked),
        };

        let ret = match pri {
            Priority::High => self.inner.quick_filter_broadcast(tar, proto_id, msg),
            Priority::Normal => self.inner.filter_broadcast(tar, proto_id, msg),
        };

        let ret = ret.map_err(|err| match &err {
            TentacleError::IoError(io_err) => match io_err.kind() {
                io::ErrorKind::BrokenPipe => NetworkError::Shutdown,
                io::ErrorKind::WouldBlock => NetworkError::Busy,
                _ => NetworkError::UnexpectedError(Box::new(err)),
            },
            _ => NetworkError::UnexpectedError(Box::new(err)),
        });

        if ret.is_err() || opt_blocked.is_some() {
            let other = ret.err();
            return Err(NetworkError::Send {
                blocked: opt_blocked,
                other: other.map(NetworkError::boxed),
            });
        }

        Ok(())
    }

    async fn users_send(
        &self,
        user_addrs: Vec<Address>,
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
