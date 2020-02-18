use std::{io, marker::PhantomData};

use async_trait::async_trait;
use futures::channel::mpsc::UnboundedSender;
use log::error;
use protocol::{traits::Priority, types::Address, Bytes};
use tentacle::{
    error::Error as TentacleError,
    service::{ServiceControl, TargetSession},
    SessionId,
};

use crate::{
    error::NetworkError,
    event::PeerManagerEvent,
    traits::{MessageSender, NetworkProtocol, SessionBook},
};

pub struct ConnectionServiceControl<P: NetworkProtocol, B: SessionBook> {
    inner:    ServiceControl,
    mgr_srv:  UnboundedSender<PeerManagerEvent>,
    sessions: B,

    // Indicate which protocol this connection service control
    pin_protocol: PhantomData<fn() -> P>,
}

impl<P: NetworkProtocol, B: SessionBook> ConnectionServiceControl<P, B> {
    pub fn new(
        control: ServiceControl,
        mgr_srv: UnboundedSender<PeerManagerEvent>,
        book: B,
    ) -> Self {
        ConnectionServiceControl {
            inner: control,
            mgr_srv,
            sessions: book,

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
                } else {
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

impl<P: NetworkProtocol, B: SessionBook + Clone> Clone for ConnectionServiceControl<P, B> {
    fn clone(&self) -> Self {
        ConnectionServiceControl {
            inner:    self.inner.clone(),
            mgr_srv:  self.mgr_srv.clone(),
            sessions: self.sessions.clone(),

            pin_protocol: PhantomData,
        }
    }
}

#[async_trait]
impl<P, B> MessageSender for ConnectionServiceControl<P, B>
where
    P: NetworkProtocol,
    B: SessionBook + Send + Sync + Unpin + 'static,
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
                other:   other.map(NetworkError::boxed),
            });
        }

        Ok(())
    }

    async fn users_send(
        &self,
        chain_addrs: Vec<Address>,
        msg: Bytes,
        pri: Priority,
    ) -> Result<(), NetworkError> {
        let (connected, unconnected) = self.sessions.by_chain(chain_addrs);

        let send_ret = self.send(TargetSession::Multi(connected), msg, pri);
        if unconnected.is_empty() {
            return send_ret;
        }

        let (pids, unknown) = self.sessions.peers_by_chain(unconnected.clone());
        let connect_peers = PeerManagerEvent::ConnectPeers { pids };
        if self.mgr_srv.unbounded_send(connect_peers).is_err() {
            error!("network: peer manager service exit");
        }

        let unconnected = unconnected
            .into_iter()
            .filter(|a| !unknown.contains(a))
            .collect::<Vec<_>>();

        if send_ret.is_err() || !unconnected.is_empty() || !unknown.is_empty() {
            let other = send_ret.err().map(NetworkError::boxed);
            let unconnected = if unconnected.is_empty() {
                None
            } else {
                Some(unconnected)
            };
            let unknown = if unknown.is_empty() {
                None
            } else {
                Some(unknown)
            };

            return Err(NetworkError::UserSend {
                unconnected,
                unknown,
                other,
            });
        }

        Ok(())
    }
}
