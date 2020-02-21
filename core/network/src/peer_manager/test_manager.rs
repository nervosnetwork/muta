use super::{
    peer::Peer, ArcPeer, Connectedness, Inner, PeerManager, PeerManagerConfig,
    ALIVE_RETRY_INTERVAL, MAX_RETRY_COUNT,
};
use crate::{
    common::ConnectedAddr,
    event::{ConnectionEvent, ConnectionType, PeerManagerEvent, RemoveKind, RetryKind},
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

    pub fn config(&self) -> &PeerManagerConfig {
        &self.inner.config
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
async fn should_not_increase_conn_count_for_connecting_peer_on_new_session() {
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
async fn should_reject_new_connections_when_we_reach_max_connections_on_new_session() {
    let (mut mgr, mut conn_rx) = make_manager(0, 10); // set max to 10
    let _remote_peers = make_sessions(&mut mgr, 10).await;

    let remote_pubkey = make_pubkey();
    let remote_addr = make_multiaddr(2077, Some(remote_pubkey.peer_id()));
    mgr.inner.unknown_addrs.insert(remote_addr.clone().into());

    let sess_ctx = SessionContext::make(
        SessionId::new(99),
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
    assert_eq!(inner.conn_count(), 10, "should not increase conn count");
    assert_eq!(
        mgr.inner.unknown_addrs.len(),
        1,
        "should not touch unknown addr"
    );

    let conn_event = conn_rx.next().await.expect("should have disconnect event");
    match conn_event {
        ConnectionEvent::Disconnect(sid) => assert_eq!(sid, 99.into(), "should be new session id"),
        _ => panic!("should be disconnect event"),
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

#[tokio::test]
async fn should_update_peer_alive_on_peer_alive() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let old_alive = test_peer.alive();

    // Set connected at to older timestamp to increase peer alive
    test_peer.set_connected_at(Peer::now() - ALIVE_RETRY_INTERVAL - 1);

    let peer_alive = PeerManagerEvent::PeerAlive {
        pid: test_peer.owned_id(),
    };
    mgr.poll_event(peer_alive).await;

    assert_eq!(
        test_peer.alive(),
        old_alive + ALIVE_RETRY_INTERVAL + 1,
        "should update peer alive"
    );
}

#[tokio::test]
async fn should_reset_peer_retry_on_peer_alive() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let test_peer = remote_peers.first().expect("get first peer");
    assert_eq!(test_peer.retry(), 0, "should have 0 retry");

    test_peer.increase_retry();
    assert_eq!(test_peer.retry(), 1, "should now have 1 retry");

    let peer_alive = PeerManagerEvent::PeerAlive {
        pid: test_peer.owned_id(),
    };
    mgr.poll_event(peer_alive).await;

    assert_eq!(test_peer.retry(), 0, "should reset retry");
}

#[tokio::test]
async fn should_disconnect_session_and_remove_peer_on_remove_peer_by_session() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let expect_sid = test_peer.session_id();
    let remove_peer_by_session = PeerManagerEvent::RemovePeerBySession {
        sid:  test_peer.session_id(),
        kind: RemoveKind::ProtocolSelect,
    };
    mgr.poll_event(remove_peer_by_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 0, "should have no session");
    assert_eq!(inner.share_sessions().len(), 0, "should have no session");
    assert!(
        inner.peer(&test_peer.id).is_none(),
        "should remove test peer"
    );

    let conn_event = conn_rx.next().await.expect("should have disconnect event");
    match conn_event {
        ConnectionEvent::Disconnect(sid) => {
            assert_eq!(sid, expect_sid, "should disconnect session")
        }
        _ => panic!("should be disconnect event"),
    }
}

#[tokio::test]
async fn should_keep_bootstrap_peer_but_max_retry_on_remove_peer_by_session() {
    let (mut mgr, mut conn_rx) = make_manager(1, 20);
    let bootstraps = &mgr.config().bootstraps;
    let test_peer = bootstraps.first().expect("get one bootstrap peer").clone();

    // Insert bootstrap peer
    let inner = mgr.core_inner();
    inner.add_peer(test_peer.clone());

    // Init bootstrap session
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

    assert_eq!(inner.conn_count(), 1, "should have one session");
    assert_eq!(
        test_peer.connectedness(),
        Connectedness::Connected,
        "should connecte"
    );
    assert!(
        test_peer.session_id() != 0.into(),
        "should not be default zero"
    );

    let expect_sid = test_peer.session_id();
    let remove_peer_by_session = PeerManagerEvent::RemovePeerBySession {
        sid:  test_peer.session_id(),
        kind: RemoveKind::ProtocolSelect,
    };
    mgr.poll_event(remove_peer_by_session).await;

    assert_eq!(inner.conn_count(), 0, "should have no session");
    assert_eq!(inner.share_sessions().len(), 0, "should have no session");
    assert!(
        inner.peer(&test_peer.id).is_some(),
        "should not remove bootstrap peer"
    );

    assert_eq!(test_peer.connectedness(), Connectedness::CanConnect);
    assert_eq!(
        test_peer.retry(),
        MAX_RETRY_COUNT,
        "should set to max retry"
    );

    let conn_event = conn_rx.next().await.expect("should have disconnect event");
    match conn_event {
        ConnectionEvent::Disconnect(sid) => {
            assert_eq!(sid, expect_sid, "should disconnect session")
        }
        _ => panic!("should be disconnect event"),
    }
}

#[tokio::test]
async fn should_mark_session_blocked_on_session_blocked() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let sess_ctx = SessionContext::make(
        test_peer.session_id(),
        test_peer.multiaddrs().pop().expect("get multiaddr"),
        SessionType::Outbound,
        test_peer.owned_pubkey(),
    );
    let session_blocked = PeerManagerEvent::SessionBlocked {
        ctx: sess_ctx.arced(),
    };
    mgr.poll_event(session_blocked).await;

    let inner = mgr.core_inner();
    let session = inner
        .session(&test_peer.session_id())
        .expect("should have a session");
    assert!(session.is_blocked(), "should be blocked");
}

#[tokio::test]
async fn should_disconnect_peer_and_increase_retry_on_retry_peer_later() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let expect_sid = test_peer.session_id();
    let retry_peer = PeerManagerEvent::RetryPeerLater {
        pid:  test_peer.owned_id(),
        kind: RetryKind::TimedOut,
    };
    mgr.poll_event(retry_peer).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.conn_count(), 0, "should have no session");
    assert_eq!(test_peer.connectedness(), Connectedness::CanConnect);
    assert_eq!(test_peer.retry(), 1, "should increase peer retry");

    let conn_event = conn_rx.next().await.expect("should have disconnect event");
    match conn_event {
        ConnectionEvent::Disconnect(sid) => {
            assert_eq!(sid, expect_sid, "should disconnect session")
        }
        _ => panic!("should be disconnect event"),
    }
}

#[tokio::test]
async fn should_try_all_peer_multiaddrs_on_connect_peers() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let peers = (0..10)
        .into_iter()
        .map(|port| {
            // Every peer has two multiaddrs
            let p = make_peer(port + 7000);
            p.add_multiaddrs(vec![make_multiaddr(port + 8000, Some(p.owned_id()))]);
            p
        })
        .collect::<Vec<_>>();

    let inner = mgr.core_inner();
    for peer in peers.iter() {
        inner.add_peer(peer.clone());
    }

    let connect_peers = PeerManagerEvent::ConnectPeers {
        pids: peers.iter().map(|p| p.owned_id()).collect(),
    };
    mgr.poll_event(connect_peers).await;

    let conn_event = conn_rx.next().await.expect("should have connect event");
    let multiaddrs_in_event = match conn_event {
        ConnectionEvent::Connect { addrs, .. } => addrs,
        _ => panic!("should be connect event"),
    };

    let expect_multiaddrs = peers
        .into_iter()
        .map(|p| p.multiaddrs())
        .flatten()
        .collect::<Vec<_>>();

    assert_eq!(
        multiaddrs_in_event.len(),
        expect_multiaddrs.len(),
        "should have same number of multiaddrs"
    );
    assert!(
        !multiaddrs_in_event
            .iter()
            .any(|ma| !expect_multiaddrs.contains(ma)),
        "all multiaddrs should be included"
    );
}

#[tokio::test]
async fn should_skip_peers_not_in_can_connect_connectedness_on_connect_peers() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let peer_in_connecting = make_peer(2077);
    let peer_in_connected = make_peer(2020);
    let peer_in_unconnectable = make_peer(2059);

    peer_in_unconnectable.set_connectedness(Connectedness::Unconnectable);
    peer_in_connected.set_connectedness(Connectedness::Connected);
    peer_in_connecting.set_connectedness(Connectedness::Connecting);

    let inner = mgr.core_inner();
    inner.add_peer(peer_in_connecting.clone());
    inner.add_peer(peer_in_connected.clone());
    inner.add_peer(peer_in_unconnectable.clone());

    let connect_peers = PeerManagerEvent::ConnectPeers {
        pids: vec![
            peer_in_unconnectable.owned_id(),
            peer_in_connected.owned_id(),
            peer_in_connecting.owned_id(),
        ],
    };
    mgr.poll_event(connect_peers).await;

    match conn_rx.try_next() {
        Err(_) => (), // Err means channel is empty, it's expected
        _ => panic!("should not have any connection event"),
    }
}

#[tokio::test]
async fn should_skip_peers_not_retry_ready_on_connect_peers() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let not_ready_peer = make_peer(2077);
    not_ready_peer.increase_retry();

    let inner = mgr.core_inner();
    inner.add_peer(not_ready_peer.clone());

    let connect_peers = PeerManagerEvent::ConnectPeers {
        pids: vec![not_ready_peer.owned_id()],
    };
    mgr.poll_event(connect_peers).await;

    match conn_rx.try_next() {
        Err(_) => (), // Err means channel is empty, it's expected
        _ => panic!("should not have any connection event"),
    }
}

// DiscoverMultiAddrs reuse DiscoverAddr logic
#[tokio::test]
async fn should_add_multiaddrs_to_unknown_book_on_discover_multi_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let test_multiaddrs: Vec<Multiaddr> = (0..10)
        .into_iter()
        .map(|port| {
            let peer = make_peer(port + 7000);
            peer.multiaddrs().pop().expect("peer multiaddr")
        })
        .collect();

    assert!(mgr.inner.unknown_addrs.is_empty());

    let discover_multi_addrs = PeerManagerEvent::DiscoverMultiAddrs {
        addrs: test_multiaddrs.clone(),
    };
    mgr.poll_event(discover_multi_addrs).await;

    let unknown_book = &mgr.inner.unknown_addrs;
    assert_eq!(unknown_book.len(), 10, "should have 10 multiaddrs");
    assert!(
        !test_multiaddrs.iter().any(|ma| !unknown_book.contains(ma)),
        "test multiaddrs should all be inserted"
    );
}

#[tokio::test]
async fn should_skip_already_exist_peer_multiaddr_on_discover_multi_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;
    let test_peer = remote_peers.first().expect("get first");

    assert!(mgr.inner.unknown_addrs.is_empty());

    let discover_multi_addrs = PeerManagerEvent::DiscoverMultiAddrs {
        addrs: vec![test_peer.multiaddrs().pop().expect("peer multiaddr")],
    };
    mgr.poll_event(discover_multi_addrs).await;

    assert!(
        mgr.inner.unknown_addrs.is_empty(),
        "should not add exist peer's multiaddr"
    );
}

#[tokio::test]
async fn should_skip_multiaddrs_without_id_included_on_discover_multi_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let test_multiaddr = make_multiaddr(2077, None);

    assert!(mgr.inner.unknown_addrs.is_empty());
    let discover_multi_addrs = PeerManagerEvent::DiscoverMultiAddrs {
        addrs: vec![test_multiaddr],
    };
    mgr.poll_event(discover_multi_addrs).await;

    assert!(
        mgr.inner.unknown_addrs.is_empty(),
        "should ignore multiaddr without id included"
    );
}

#[tokio::test]
async fn should_add_multiaddrs_to_peer_on_identified_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;
    let test_peer = remote_peers.first().expect("get first");
    let old_multiaddrs_len = test_peer.multiaddrs_len();

    let test_multiaddrs: Vec<_> = (0..2)
        .into_iter()
        .map(|port| make_multiaddr(port + 9000, Some(test_peer.owned_id())))
        .collect();

    let identified_addrs = PeerManagerEvent::IdentifiedAddrs {
        pid:   test_peer.owned_id(),
        addrs: test_multiaddrs.clone(),
    };
    mgr.poll_event(identified_addrs).await;

    assert_eq!(
        test_peer.multiaddrs_len(),
        old_multiaddrs_len + 2,
        "should have correct multiaddrs len"
    );
    assert!(
        !test_multiaddrs
            .iter()
            .any(|ma| !test_peer.multiaddrs().contains(ma)),
        "should add all multiaddrs to peer"
    );
}

#[tokio::test]
async fn should_push_id_to_multiaddrs_if_not_included_on_identified_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;
    let test_peer = remote_peers.first().expect("get first");
    let test_multiaddr = make_multiaddr(2077, None);

    let identified_addrs = PeerManagerEvent::IdentifiedAddrs {
        pid:   test_peer.owned_id(),
        addrs: vec![test_multiaddr.clone()],
    };
    mgr.poll_event(identified_addrs).await;

    assert!(
        !test_peer.multiaddrs().contains(&test_multiaddr),
        "should not contain multiaddr without id included"
    );

    let with_id = make_multiaddr(2077, Some(test_peer.owned_id()));
    assert!(
        test_peer.multiaddrs().contains(&with_id),
        "should push id to multiaddr when add it to peer"
    );
}

#[tokio::test]
async fn should_add_dialer_multiaddr_to_peer_on_repeated_connection() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;
    let test_peer = remote_peers.first().expect("get first");
    let test_multiaddr = make_multiaddr(2077, Some(test_peer.owned_id()));

    let repeated_connection = PeerManagerEvent::RepeatedConnection {
        ty:   ConnectionType::Dialer,
        sid:  test_peer.session_id(),
        addr: test_multiaddr.clone(),
    };
    mgr.poll_event(repeated_connection).await;

    assert!(
        test_peer.multiaddrs().contains(&test_multiaddr),
        "should add dialer multiaddr to peer"
    );
}

#[tokio::test]
async fn should_skip_listen_multiaddr_to_peer_on_repeated_connection() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;
    let test_peer = remote_peers.first().expect("get first");
    let test_multiaddr = make_multiaddr(2077, Some(test_peer.owned_id()));

    let repeated_connection = PeerManagerEvent::RepeatedConnection {
        ty:   ConnectionType::Listen,
        sid:  test_peer.session_id(),
        addr: test_multiaddr.clone(),
    };
    mgr.poll_event(repeated_connection).await;

    assert!(
        !test_peer.multiaddrs().contains(&test_multiaddr),
        "should skip listen multiaddr to peer"
    );
}

#[tokio::test]
async fn should_push_id_if_multiaddr_not_included_on_repeated_connection() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;
    let test_peer = remote_peers.first().expect("get first");
    let test_multiaddr = make_multiaddr(2077, None);

    let repeated_connection = PeerManagerEvent::RepeatedConnection {
        ty:   ConnectionType::Dialer,
        sid:  test_peer.session_id(),
        addr: test_multiaddr.clone(),
    };
    mgr.poll_event(repeated_connection).await;

    assert!(
        !test_peer.multiaddrs().contains(&test_multiaddr),
        "should not add multiaddr without id included"
    );

    let with_id = make_multiaddr(2077, Some(test_peer.owned_id()));
    assert!(
        test_peer.multiaddrs().contains(&with_id),
        "should add multiaddr wit id included"
    );
}

#[tokio::test]
async fn should_remove_multiaddr_in_unknown_book_on_repeated_connection() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1).await;
    let test_peer = remote_peers.first().expect("get first");
    let test_multiaddr = make_multiaddr(2077, None);
    let test_multiaddr_with_id = make_multiaddr(2077, Some(test_peer.owned_id()));

    mgr.inner
        .unknown_addrs
        .insert(test_multiaddr.clone().into());
    mgr.inner
        .unknown_addrs
        .insert(test_multiaddr_with_id.clone().into());
    assert_eq!(
        mgr.inner.unknown_addrs.len(),
        2,
        "should have 2 unknown multiaddrs"
    );

    let repeated_connection = PeerManagerEvent::RepeatedConnection {
        ty:   ConnectionType::Dialer,
        sid:  test_peer.session_id(),
        addr: test_multiaddr.clone(),
    };
    mgr.poll_event(repeated_connection).await;

    assert!(
        mgr.inner.unknown_addrs.is_empty(),
        "should remove multiaddrs in unknown with/without id included"
    );
}
