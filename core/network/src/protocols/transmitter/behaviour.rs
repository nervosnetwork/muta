use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::task::{Context, Poll};

use arc_swap::ArcSwapOption;
use dashmap::DashMap;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;
use futures::stream::Stream;
use protocol::traits::Priority;
use protocol::Bytes;
use tentacle::error::SendErrorKind;
use tentacle::secio::PeerId;
use tentacle::service::TargetSession;
use tentacle::SessionId;

use super::message::{Recipient, TransmitterMessage};

use crate::connection::{ConnectionServiceControl, ProtocolMessage};
use crate::error::{ErrorKind, NetworkError};
use crate::event::PeerManagerEvent;
use crate::peer_manager::SharedSessions;
use crate::protocols::core::TRANSMITTER_PROTOCOL_ID;
use crate::traits::SessionBook;

// TODO: Refactor connection service, decouple protocol and service
// initialization.
#[derive(Clone)]
pub struct TransmitterBehaviour {
    pending_sending_tx: ArcSwapOption<UnboundedSender<PendingSending>>,
}

impl TransmitterBehaviour {
    pub fn new() -> Self {
        let pending_sending_tx = ArcSwapOption::from(None);

        TransmitterBehaviour { pending_sending_tx }
    }

    pub fn init(
        &self,
        conn_ctrl: ConnectionServiceControl,
        peers_serv: UnboundedSender<PeerManagerEvent>,
        sessions: SharedSessions,
    ) {
        let (pending_sending_tx, pending_sending_rx) = mpsc::unbounded();

        let background_sending = BackgroundSending {
            conn_ctrl,
            peers_serv,
            sessions,
            session_seq: DashMap::new(),
            pending_sending_rx,
        };
        tokio::spawn(background_sending);

        self.pending_sending_tx
            .store(Some(Arc::new(pending_sending_tx)))
    }

    pub fn send(&self, msg: TransmitterMessage) -> impl Future<Output = Result<(), NetworkError>> {
        let (tx, rx) = oneshot::channel();

        let pending_sending = PendingSending { msg, tx };
        let tx_guard = self.pending_sending_tx.load();

        async move {
            match tx_guard.as_ref() {
                Some(tx) => {
                    if let Err(e) = tx.unbounded_send(pending_sending) {
                        log::error!("pending sending tx dropped");
                        return Err(NetworkError::Internal(Box::new(e)));
                    }
                }
                None => {
                    log::error!("transmitter behaviour isn't inited");
                    return Err(NetworkError::Internal(Box::new(ErrorKind::Internal(
                        "transmitter behaviour isn't inited".to_owned(),
                    ))));
                }
            }

            match rx.await {
                Ok(Err(e)) => Err(NetworkError::Internal(Box::new(e))),
                Err(e) => Err(NetworkError::Internal(Box::new(e))),
                Ok(Ok(_)) => Ok(()),
            }
        }
    }
}

struct PendingSending {
    msg: TransmitterMessage,
    tx:  oneshot::Sender<Result<(), NetworkError>>,
}

struct BackgroundSending {
    conn_ctrl:          ConnectionServiceControl,
    peers_serv:         UnboundedSender<PeerManagerEvent>,
    sessions:           SharedSessions,
    session_seq:        DashMap<SessionId, AtomicU64>, /* Maintain data seq number for every
                                                        * session */
    pending_sending_rx: UnboundedReceiver<PendingSending>,
}

impl BackgroundSending {
    pub fn context(&self) -> SendingContext<'_> {
        SendingContext {
            conn_ctrl:   &self.conn_ctrl,
            peers_serv:  &self.peers_serv,
            sessions:    &self.sessions,
            session_seq: &self.session_seq,
        }
    }
}

impl Future for BackgroundSending {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let pending_sending_rx = &mut self.as_mut().pending_sending_rx;
            futures::pin_mut!(pending_sending_rx);

            match futures::ready!(pending_sending_rx.poll_next(ctx)) {
                Some(PendingSending { msg, tx }) => {
                    if let Err(e) = tx.send(self.context().send(msg)) {
                        log::warn!("pending sending result {:?}", e);
                    }
                }
                None => {
                    log::error!("transmitter pending tx dropped");
                    return Poll::Ready(());
                }
            }
        }
    }
}

struct SendingContext<'a> {
    conn_ctrl:   &'a ConnectionServiceControl,
    peers_serv:  &'a UnboundedSender<PeerManagerEvent>,
    sessions:    &'a SharedSessions,
    session_seq: &'a DashMap<SessionId, AtomicU64>,
}

impl<'a> SendingContext<'a> {
    fn send(&self, msg: TransmitterMessage) -> Result<(), NetworkError> {
        let TransmitterMessage { priority, data, .. } = msg;

        match msg.recipient {
            Recipient::Session(target) => self.send_to_sessions(target, data, priority),
            Recipient::PeerId(peer_ids) => self.send_to_peers(peer_ids, data, priority),
        }
    }

    fn send_to_sessions(
        &self,
        target: TargetSession,
        data: Bytes,
        priority: Priority,
    ) -> Result<(), NetworkError> {
        let (target, opt_blocked) = match self.filter_blocked(target) {
            (None, None) => unreachable!(),
            (None, blocked) => {
                return Err(NetworkError::Send {
                    blocked,
                    other: None,
                });
            }
            (Some(tar), opt_blocked) => (tar, opt_blocked),
        };

        let proto_msg = ProtocolMessage {
            protocol_id: TRANSMITTER_PROTOCOL_ID.into(),
            target,
            data,
            priority,
        };

        let ret = self.conn_ctrl.send(proto_msg).map_err(|err| match &err {
            SendErrorKind::BrokenPipe => NetworkError::Shutdown,
            SendErrorKind::WouldBlock => NetworkError::Busy,
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

    fn send_to_peers(
        &self,
        peer_ids: Vec<PeerId>,
        data: Bytes,
        priority: Priority,
    ) -> Result<(), NetworkError> {
        let (connected, unconnected) = self.sessions.peers(peer_ids);
        let send_ret = self.send_to_sessions(TargetSession::Multi(connected), data, priority);
        if unconnected.is_empty() {
            return send_ret;
        }

        let connect_peers = PeerManagerEvent::ConnectPeersNow {
            pids: unconnected.clone(),
        };
        if self.peers_serv.unbounded_send(connect_peers).is_err() {
            log::error!("network: peer manager service exit");
        }

        if send_ret.is_err() || !unconnected.is_empty() {
            let other = send_ret.err().map(NetworkError::boxed);
            let unconnected = if unconnected.is_empty() {
                None
            } else {
                Some(unconnected)
            };

            return Err(NetworkError::MultiCast { unconnected, other });
        }

        Ok(())
    }

    fn filter_blocked(
        &self,
        target: TargetSession,
    ) -> (Option<TargetSession>, Option<Vec<SessionId>>) {
        self.sessions.refresh_blocked();

        let all_blocked = self.sessions.all_blocked();
        if all_blocked.is_empty() {
            return (Some(target), None);
        }

        match target {
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
