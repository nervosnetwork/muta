use super::{
    peer::Peer, ArcPeer, Connectedness, Inner, PeerManager, PeerManagerConfig, ALIVE_RETRY_INTERVAL,
};
use crate::{
    common::ConnectedAddr,
    event::{ConnectionEvent, PeerManagerEvent},
    test::mock::SessionContext,
    traits::MultiaddrExt,
};

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use tentacle::{
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey, SecioKeyPair},
    service::SessionType,
    SessionId,
};

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

fn make_multiaddr(port: u16, id: Option<PeerId>) -> Multiaddr {
    let mut multiaddr = format!("/ip4/127.0.0.1/tcp/{}", port)
        .parse::<Multiaddr>()
        .expect("peer multiaddr");

    if let Some(id) = id {
        multiaddr.push_id(id);
    }

    multiaddr
}

fn make_peer(port: u16) -> ArcPeer {
    let keypair = SecioKeyPair::secp256k1_generated();
    let pubkey = keypair.public_key();
    let peer_id = pubkey.peer_id();
    let peer = ArcPeer::from_pubkey(pubkey).expect("make peer");
    let multiaddr = make_multiaddr(port, Some(peer_id));

    peer.set_multiaddrs(vec![multiaddr]);
    peer
}

fn make_bootstraps(num: usize) -> Vec<ArcPeer> {
    let mut init_port = 5000;

    (0..num)
        .into_iter()
        .map(|_| {
            let peer = make_peer(init_port);
            init_port += 1;
            peer
        })
        .collect()
}

struct MockManager {
    event_tx: UnboundedSender<PeerManagerEvent>,
    mgr:      PeerManager,
}

impl MockManager {
    pub fn new(mgr: PeerManager, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        MockManager { event_tx, mgr }
    }

    pub async fn poll_event(&mut self, event: PeerManagerEvent) {
        self.event_tx.unbounded_send(event).expect("send event");
        self.await
    }

    pub fn inner(&self) -> Arc<Inner> {
        self.mgr.inner()
    }
}

impl Future for MockManager {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let _ = Future::poll(Pin::new(&mut self.as_mut().mgr), ctx);
        Poll::Ready(())
    }
}

fn make_manager(
    pubkey: PublicKey,
    bootstrap_num: usize,
) -> (MockManager, UnboundedReceiver<ConnectionEvent>) {
    let our_id = pubkey.peer_id();
    let bootstraps = make_bootstraps(bootstrap_num);
    let mut peer_dat_file = std::env::temp_dir();
    peer_dat_file.push("peer.dat");

    let config = PeerManagerConfig {
        our_id,
        pubkey,
        bootstraps,
        max_connections: 20,
        routine_interval: Duration::from_secs(10),
        peer_dat_file,
    };

    let (conn_tx, conn_rx) = unbounded();
    let (mgr_tx, mgr_rx) = unbounded();
    let manager = PeerManager::new(config, mgr_rx, conn_tx);

    (MockManager::new(manager, mgr_tx), conn_rx)
}

fn make_pubkey() -> PublicKey {
    let keypair = SecioKeyPair::secp256k1_generated();
    keypair.public_key()
}

#[tokio::test]
async fn test_new_session() {
    let our_pubkey = make_pubkey();
    let (mut mgr, _conn_rx) = make_manager(our_pubkey, 2);

    let remote_pubkey = make_pubkey();
    let remote_addr = make_multiaddr(6000, Some(remote_pubkey.peer_id()));
    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        remote_addr.clone(),
        SessionType::Inbound,
        remote_pubkey.clone(),
    );

    let event_new_session = PeerManagerEvent::NewSession {
        pid:    remote_pubkey.peer_id(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(event_new_session).await;

    let inner = mgr.inner();
    assert_eq!(inner.conn_count(), 1, "should have one without bootstrap");

    let saved_peer = inner
        .peer(&remote_pubkey.peer_id())
        .expect("should save peer");
    assert_eq!(saved_peer.session_id(), 1.into());
    assert_eq!(
        saved_peer.multiaddrs_len(),
        0,
        "should not save inbound multiaddr"
    );
    assert_eq!(saved_peer.connectedness(), Connectedness::Connected);

    let saved_session = inner.session(&1.into()).expect("should save session");
    assert_eq!(saved_session.peer.id.as_ref(), &remote_pubkey.peer_id());
    assert!(!saved_session.is_blocked());
    assert_eq!(
        saved_session.connected_addr,
        ConnectedAddr::from(&remote_addr)
    );
}

#[tokio::test]
async fn test_session_closed() {
    let our_pubkey = make_pubkey();
    let (mut mgr, _conn_rx) = make_manager(our_pubkey, 2);

    let remote_pubkey = make_pubkey();
    let remote_addr = make_multiaddr(6000, Some(remote_pubkey.peer_id()));
    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        remote_addr.clone(),
        SessionType::Inbound,
        remote_pubkey.clone(),
    );

    let event_new_session = PeerManagerEvent::NewSession {
        pid:    remote_pubkey.peer_id(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.clone().arced(),
    };
    mgr.poll_event(event_new_session).await;

    let inner = mgr.inner();
    assert_eq!(inner.conn_count(), 1, "should have one without bootstrap");

    let event_session_closed = PeerManagerEvent::SessionClosed {
        pid: remote_pubkey.peer_id(),
        sid: 1.into(),
    };
    mgr.poll_event(event_session_closed).await;

    let inner = mgr.inner();
    assert_eq!(inner.conn_count(), 0, "should have no session");
    assert_eq!(inner.share_sessions().len(), 0, "should have no session");

    let saved_peer = inner
        .peer(&remote_pubkey.peer_id())
        .expect("should save peer");
    assert_eq!(saved_peer.connectedness(), Connectedness::CanConnect);
    assert_eq!(
        saved_peer.retry(),
        1,
        "should increase retry count, short alive"
    );

    let event_new_session = PeerManagerEvent::NewSession {
        pid:    remote_pubkey.peer_id(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(event_new_session).await;
    assert_eq!(saved_peer.retry(), 0, "should reset retry after connect");
    saved_peer.set_connected_at(Peer::now() - ALIVE_RETRY_INTERVAL - 1);

    let event_session_closed = PeerManagerEvent::SessionClosed {
        pid: remote_pubkey.peer_id(),
        sid: 1.into(),
    };
    mgr.poll_event(event_session_closed).await;
    assert_eq!(inner.conn_count(), 0, "should have no session");
    assert_eq!(saved_peer.retry(), 0, "should keep retry to 0");
}
