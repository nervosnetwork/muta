use super::{
    peer::Peer, ArcPeer, Connectedness, Inner, PeerManager, PeerManagerConfig, ALIVE_RETRY_INTERVAL,
};
use crate::{
    common::ConnectedAddr,
    event::{ConnectionEvent, PeerManagerEvent},
    test::mock::SessionContext,
    traits::MultiaddrExt,
};

use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
    StreamExt,
};
use tentacle::{
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey, SecioKeyPair},
    service::SessionType,
    SessionId,
};

use std::{
    borrow::Cow,
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
    inner:    PeerManager,
}

impl MockManager {
    pub fn new(inner: PeerManager, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        MockManager { event_tx, inner }
    }

    pub async fn poll_event(&mut self, event: PeerManagerEvent) {
        self.event_tx.unbounded_send(event).expect("send event");
        self.await
    }

    pub async fn poll(&mut self) {
        self.await
    }

    pub fn core_inner(&self) -> Arc<Inner> {
        self.inner.inner()
    }
}

impl Future for MockManager {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let _ = Future::poll(Pin::new(&mut self.as_mut().inner), ctx);
        Poll::Ready(())
    }
}

fn make_manager(
    bootstrap_num: usize,
    max_connections: usize,
) -> (MockManager, UnboundedReceiver<ConnectionEvent>) {
    let manager_pubkey = make_pubkey();
    let manager_id = manager_pubkey.peer_id();
    let bootstraps = make_bootstraps(bootstrap_num);
    let mut peer_dat_file = std::env::temp_dir();
    peer_dat_file.push("peer.dat");

    let config = PeerManagerConfig {
        our_id: manager_id,
        pubkey: manager_pubkey,
        bootstraps,
        max_connections,
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

async fn make_sessions(mgr: &mut MockManager, num: usize) -> Vec<ArcPeer> {
    let mut next_sid = 1;
    let mut peers = Vec::with_capacity(num);
    let inner = mgr.core_inner();

    for _ in (0..num).into_iter() {
        let remote_pubkey = make_pubkey();
        let remote_pid = remote_pubkey.peer_id();
        let remote_addr = make_multiaddr(6000, Some(remote_pid.clone()));

        let sess_ctx = SessionContext::make(
            SessionId::new(next_sid),
            remote_addr.clone(),
            SessionType::Outbound,
            remote_pubkey.clone(),
        );
        next_sid += 1;

        let new_session = PeerManagerEvent::NewSession {
            pid:    remote_pid.clone(),
            pubkey: remote_pubkey,
            ctx:    sess_ctx.arced(),
        };
        mgr.poll_event(new_session).await;

        peers.push(inner.peer(&remote_pid).expect("make peer session"));
    }

    assert_eq!(inner.conn_count(), num, "make some sessions");
    peers
}

#[tokio::test]
async fn should_accept_outbound_new_session_and_add_peer() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);

    let remote_pubkey = make_pubkey();
    let remote_addr = make_multiaddr(6000, Some(remote_pubkey.peer_id()));
    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        remote_addr.clone(),
        SessionType::Outbound,
        remote_pubkey.clone(),
    );

    let new_session = PeerManagerEvent::NewSession {
        pid:    remote_pubkey.peer_id(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 1, "should have one without bootstrap");

    let saved_peer = inner
        .peer(&remote_pubkey.peer_id())
        .expect("should save peer");
    assert_eq!(saved_peer.session_id(), 1.into());
    assert_eq!(saved_peer.connectedness(), Connectedness::Connected);
    let saved_addrs_len = saved_peer.multiaddrs_len();
    assert_eq!(saved_addrs_len, 1, "should save outbound multiaddr");
    assert!(
        saved_peer.multiaddrs().contains(&remote_addr),
        "should contain remote multiadr"
    );
    assert_eq!(saved_peer.retry(), 0, "should reset retry");

    let saved_session = inner.session(&1.into()).expect("should save session");
    assert_eq!(saved_session.peer.id.as_ref(), &remote_pubkey.peer_id());
    assert!(!saved_session.is_blocked());
    assert_eq!(
        saved_session.connected_addr,
        ConnectedAddr::from(&remote_addr)
    );
}

#[tokio::test]
async fn should_ignore_inbound_address_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);

    let remote_pubkey = make_pubkey();
    let remote_addr = make_multiaddr(6000, Some(remote_pubkey.peer_id()));
    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        remote_addr.clone(),
        SessionType::Inbound,
        remote_pubkey.clone(),
    );

    let new_session = PeerManagerEvent::NewSession {
        pid:    remote_pubkey.peer_id(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 1, "should have one without bootstrap");

    let saved_peer = inner
        .peer(&remote_pubkey.peer_id())
        .expect("should save peer");
    let saved_addrs_len = saved_peer.multiaddrs_len();
    assert_eq!(saved_addrs_len, 0, "should not save inbound multiaddr");
}

#[tokio::test]
async fn should_enforce_id_in_multiaddr_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);

    let remote_pubkey = make_pubkey();
    let remote_addr = make_multiaddr(6000, None);
    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        remote_addr.clone(),
        SessionType::Outbound,
        remote_pubkey.clone(),
    );

    let new_session = PeerManagerEvent::NewSession {
        pid:    remote_pubkey.peer_id(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 1, "should have one without bootstrap");

    let saved_peer = inner
        .peer(&remote_pubkey.peer_id())
        .expect("should save peer");
    let saved_addrs = saved_peer.multiaddrs();
    assert_eq!(saved_addrs.len(), 1, "should save outbound multiaddr");

    let remote_addr = saved_addrs.first().expect("get first multiaddr");
    assert!(remote_addr.has_id());
    assert_eq!(
        remote_addr.id_bytes(),
        Some(Cow::Borrowed(remote_pubkey.peer_id().as_bytes())),
        "id should match"
    );
}

#[tokio::test]
async fn should_add_multiaddr_to_peer_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 1, "should have one without bootstrap");

    let test_peer = remote_peers.first().expect("get first");
    let session_closed = PeerManagerEvent::SessionClosed {
        pid: test_peer.owned_id(),
        sid: test_peer.session_id(),
    };
    mgr.poll_event(session_closed).await;

    let new_multiaddr = make_multiaddr(9999, None);
    let sess_ctx = SessionContext::make(
        SessionId::new(2),
        new_multiaddr,
        SessionType::Outbound,
        test_peer.owned_pubkey(),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    assert_eq!(test_peer.multiaddrs_len(), 2, "should have 2 addrs");
}

#[tokio::test]
async fn should_not_increase_conn_count_for_connecting_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let test_peer = make_peer(2020);

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 0, "should not have any connection");

    inner.add_peer(test_peer.clone());
    mgr.poll().await;
    assert_eq!(inner.conn_count(), 1, "should try connect test peer");
    assert_eq!(test_peer.connectedness(), Connectedness::Connecting);

    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        test_peer.multiaddrs().pop().expect("get multiaddr"),
        SessionType::Outbound,
        test_peer.owned_pubkey(),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    assert_eq!(test_peer.connectedness(), Connectedness::Connected);
    assert_eq!(
        inner.conn_count(),
        1,
        "should not increase conn count on connecting"
    );
}

#[tokio::test]
async fn should_remove_same_multiaddr_in_unknown_book_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let test_addr = make_multiaddr(2077, None);
    mgr.inner.unknown_addrs.insert(test_addr.clone().into());
    assert_eq!(
        mgr.inner.unknown_addrs.len(),
        1,
        "should have one unknown addr"
    );

    let remote_pubkey = make_pubkey();
    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        test_addr,
        SessionType::Outbound,
        remote_pubkey.clone(),
    );

    let new_session = PeerManagerEvent::NewSession {
        pid:    remote_pubkey.peer_id(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 1, "should have one connection");
    assert_eq!(
        mgr.inner.unknown_addrs.len(),
        0,
        "should remove same unknown addr"
    );
}

#[tokio::test]
async fn should_remove_mutiaddr_with_id_in_unknown_book_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);

    let remote_pubkey = make_pubkey();
    let test_addr = make_multiaddr(2077, None);
    let test_id_addr = make_multiaddr(2077, Some(remote_pubkey.peer_id()));
    mgr.inner.unknown_addrs.insert(test_id_addr.into());
    assert_eq!(
        mgr.inner.unknown_addrs.len(),
        1,
        "should have one unknown addr"
    );

    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        test_addr,
        SessionType::Outbound,
        remote_pubkey.clone(),
    );

    let new_session = PeerManagerEvent::NewSession {
        pid:    remote_pubkey.peer_id(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 1, "should have one connection");
    assert_eq!(
        mgr.inner.unknown_addrs.len(),
        0,
        "should remove same unknown addr"
    );
}

#[tokio::test]
async fn should_reject_new_connection_for_same_peer_on_new_session() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let expect_sid = test_peer.session_id();
    let sess_ctx = SessionContext::make(
        SessionId::new(99),
        test_peer.multiaddrs().pop().expect("get multiaddr"),
        SessionType::Outbound,
        test_peer.owned_pubkey(),
    );

    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 1, "should not increase conn count");
    assert_eq!(
        test_peer.session_id(),
        expect_sid,
        "should not change peer session id"
    );

    let conn_event = conn_rx.next().await.expect("should have disconnect event");
    match conn_event {
        ConnectionEvent::Disconnect(sid) => assert_eq!(sid, 99.into(), "should be new session id"),
        _ => panic!("should be disconnect event"),
    }
}

#[tokio::test]
async fn should_keep_new_connection_for_error_outdated_peer_session_on_new_session() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let inner = mgr.core_inner();
    let test_peer = remote_peers.first().expect("get first peer");
    inner.remove_session(&test_peer.session_id());

    let sess_ctx = SessionContext::make(
        SessionId::new(99),
        test_peer.multiaddrs().pop().expect("get multiaddr"),
        SessionType::Outbound,
        test_peer.owned_pubkey(),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    assert_eq!(inner.conn_count(), 1, "should not increase conn count");
    assert_eq!(
        test_peer.session_id(),
        99.into(),
        "should update session id"
    );

    match conn_rx.try_next() {
        Err(_) => (), // Err means channel is empty, it's expected
        _ => panic!("should not have any connection event"),
    }
}

#[tokio::test]
async fn should_remove_session_on_session_closed() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let test_peer = remote_peers.first().expect("get first peer");
    assert_eq!(test_peer.retry(), 0, "should reset retry after connect");
    // Set connected at to older timestamp to increase peer alive
    test_peer.set_connected_at(Peer::now() - ALIVE_RETRY_INTERVAL - 1);

    let session_closed = PeerManagerEvent::SessionClosed {
        pid: test_peer.owned_id(),
        sid: test_peer.session_id(),
    };
    mgr.poll_event(session_closed).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 1, "should have one connecting session");
    assert_eq!(inner.share_sessions().len(), 0, "should have no session");
    assert_eq!(
        test_peer.connectedness(),
        Connectedness::Connecting,
        "should try reconnect since we aren't reach max connections"
    );
    assert_eq!(test_peer.retry(), 0, "should keep retry to 0");
}

#[tokio::test]
async fn should_increase_retry_for_short_alive_session_on_session_closed() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let test_peer = remote_peers.first().expect("get first peer");
    assert_eq!(test_peer.retry(), 0, "should reset retry after connect");

    let session_closed = PeerManagerEvent::SessionClosed {
        pid: test_peer.owned_id(),
        sid: test_peer.session_id(),
    };
    mgr.poll_event(session_closed).await;

    let inner = mgr.core_inner();
    assert_eq!(
        inner.conn_count(),
        0,
        "should have no session because of retry"
    );
    assert_eq!(inner.share_sessions().len(), 0, "should have no session");
    assert_eq!(test_peer.connectedness(), Connectedness::CanConnect);
    assert_eq!(test_peer.retry(), 1, "should increase retry count");
}
