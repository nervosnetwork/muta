use super::{
    time, ArcPeer, Connectedness, ConnectingAttempt, Inner, MisbehaviorKind, PeerManager,
    PeerManagerConfig, PeerMultiaddr, TestExpireTime, TrustMetricConfig, MAX_RETRY_COUNT,
    REPEATED_CONNECTION_TIMEOUT, SHORT_ALIVE_SESSION, WHITELIST_TIMEOUT,
};
use crate::{
    common::ConnectedAddr,
    event::{
        ConnectionErrorKind, ConnectionEvent, ConnectionType, PeerManagerEvent, SessionErrorKind,
    },
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
    collections::HashSet,
    convert::TryInto,
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

fn make_peer_multiaddr(port: u16, id: PeerId) -> PeerMultiaddr {
    make_multiaddr(port, Some(id))
        .try_into()
        .expect("try into peer multiaddr")
}

fn make_peer(port: u16) -> ArcPeer {
    let keypair = SecioKeyPair::secp256k1_generated();
    let pubkey = keypair.public_key();
    let peer_id = pubkey.peer_id();
    let peer = ArcPeer::from_pubkey(pubkey).expect("make peer");
    let multiaddr = make_peer_multiaddr(port, peer_id);

    peer.multiaddrs.set(vec![multiaddr]);
    peer
}

fn make_bootstraps(num: usize) -> Vec<ArcPeer> {
    let mut init_port = 5000;

    (0..num)
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

    pub fn connecting(&self) -> &HashSet<ConnectingAttempt> {
        &self.inner.connecting
    }

    pub fn connecting_mut(&mut self) -> &mut HashSet<ConnectingAttempt> {
        &mut self.inner.connecting
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
    let peer_trust_config = Arc::new(TrustMetricConfig::default());

    let config = PeerManagerConfig {
        our_id: manager_id,
        pubkey: manager_pubkey,
        bootstraps,
        whitelist_by_chain_addrs: Default::default(),
        whitelist_peers_only: false,
        peer_trust_config,
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

async fn make_sessions(mgr: &mut MockManager, num: u16, init_port: u16) -> Vec<ArcPeer> {
    let mut next_sid = 1;
    let mut peers = Vec::with_capacity(num as usize);
    let inner = mgr.core_inner();

    for n in (0..num).into_iter() {
        let remote_pubkey = make_pubkey();
        let remote_pid = remote_pubkey.peer_id();
        let remote_addr = make_multiaddr(init_port + n, Some(remote_pid.clone()));

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

    assert_eq!(inner.connected(), num as usize, "make some sessions");
    peers
}

#[tokio::test]
async fn should_accept_new_peer_inbound_connection_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);

    let remote_pubkey = make_pubkey();
    let remote_peer_id = remote_pubkey.peer_id();
    let remote_addr = make_multiaddr(6000, Some(remote_pubkey.peer_id()));

    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        remote_addr.clone(),
        SessionType::Inbound,
        remote_pubkey.clone(),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    remote_peer_id.clone(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 1, "should have one without bootstrap");

    let saved_peer = inner.peer(&remote_peer_id).expect("should save peer");
    assert_eq!(saved_peer.session_id(), 1.into());
    assert!(saved_peer.has_pubkey(), "should have public key");
    assert!(
        saved_peer.owned_chain_addr().is_some(),
        "should have chain addr"
    );
    assert_eq!(saved_peer.connectedness(), Connectedness::Connected);
    assert_eq!(saved_peer.retry.count(), 0, "should reset retry");

    let saved_session = inner.session(1.into()).expect(
        "should save
session",
    );
    assert_eq!(saved_session.peer.id, remote_pubkey.peer_id());
    assert!(!saved_session.is_blocked());
    assert_eq!(
        saved_session.connected_addr,
        ConnectedAddr::from(&remote_addr)
    );
}

#[tokio::test]
async fn should_accept_outbound_connection_and_remove_mached_connecting_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);

    let test_peer = make_peer(9527);
    let test_multiaddr = test_peer.multiaddrs.all_raw().pop().expect("get multiaddr");
    let target_attempt = ConnectingAttempt::new(test_peer.clone());

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 0, "should have zero connected");

    mgr.connecting_mut().insert(target_attempt);
    assert_eq!(
        mgr.connecting().len(),
        1,
        "should have one connecting
attempt"
    );

    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        test_multiaddr.clone(),
        SessionType::Outbound,
        test_peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    assert_eq!(
        mgr.connecting().len(),
        0,
        "should have 0 connecting attempt"
    );
    assert_eq!(inner.connected(), 1, "should have 1 connected");
    assert!(inner.peer(&test_peer.id).is_some(), "should match peer");
}

#[tokio::test]
async fn should_set_matched_peer_pubkey_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(0, 2);

    let inner = mgr.core_inner();
    let test_pubkey = make_pubkey();
    let test_peer = ArcPeer::new(test_pubkey.peer_id());
    inner.add_peer(test_peer.clone());

    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        make_multiaddr(9527, None),
        SessionType::Outbound,
        test_pubkey.clone(),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_pubkey.peer_id(),
        pubkey: test_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 1, "should one connection");
    assert_eq!(
        test_peer.owned_pubkey(),
        Some(test_pubkey),
        "should set peer pubkey"
    );
}

#[tokio::test]
async fn should_reset_outbound_peer_multiaddr_failure_count_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(0, 2);

    let inner = mgr.core_inner();
    let test_peer = make_peer(9527);
    inner.add_peer(test_peer.clone());

    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("test multiaddr");
    test_peer.multiaddrs.inc_failure(&test_multiaddr);
    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(1),
        "should have one failure"
    );

    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        make_multiaddr(9527, None),
        SessionType::Outbound,
        test_peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 1, "should one connection");
    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(0),
        "should reset matched outbound multiaddr's failure"
    );
}

#[tokio::test]
async fn should_ignore_inbound_address_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);

    let remote_pubkey = make_pubkey();
    let remote_peer_id = remote_pubkey.peer_id();
    let remote_addr = make_multiaddr(6000, Some(remote_pubkey.peer_id()));

    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        remote_addr.clone(),
        SessionType::Inbound,
        remote_pubkey.clone(),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    remote_peer_id.clone(),
        pubkey: remote_pubkey.clone(),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 1, "should have one without bootstrap");

    let saved_peer = inner.peer(&remote_peer_id).expect("should save peer");
    assert_eq!(
        saved_peer.multiaddrs.len(),
        0,
        "should not save inbound multiaddr"
    );
}

#[tokio::test]
async fn should_enforce_id_in_multiaddr_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);

    let remote_pubkey = make_pubkey();
    let remote_peer_id = remote_pubkey.peer_id();
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
    assert_eq!(inner.connected(), 1, "should have one without bootstrap");

    let saved_peer = inner.peer(&remote_peer_id).expect("should save peer");
    let saved_addrs = saved_peer.multiaddrs.all_raw();
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
async fn should_add_new_outbound_multiaddr_to_peer_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 1, "should have one without bootstrap");

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
        test_peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    assert_eq!(test_peer.multiaddrs.len(), 2, "should have 2 addrs");

    let test_peer_multiaddr = make_peer_multiaddr(9999, test_peer.owned_id());
    assert!(
        test_peer.multiaddrs.contains(&test_peer_multiaddr),
        "should have this new multiaddr"
    );
}

#[tokio::test]
async fn should_always_remove_inbound_multiaddr_even_if_we_reach_max_connections_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(0, 2);
    let _remote_peers = make_sessions(&mut mgr, 2, 5000).await;

    let inner = mgr.core_inner();
    let test_peer = make_peer(9527);
    inner.add_peer(test_peer.clone());
    assert_eq!(
        test_peer.multiaddrs.len(),
        1,
        "should have on inbound address"
    );

    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        make_multiaddr(9527, Some(test_peer.owned_id())),
        SessionType::Inbound,
        test_peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 2, "should not increase conn count");

    assert_eq!(
        test_peer.multiaddrs.len(),
        0,
        "should remove inbound address"
    );
}

#[tokio::test]
async fn should_remove_matched_peer_inbound_address_from_ctx_even_if_it_doesnt_have_id_on_new_session(
) {
    let (mut mgr, _conn_rx) = make_manager(0, 2);

    let inner = mgr.core_inner();
    let test_peer = make_peer(9527);
    inner.add_peer(test_peer.clone());
    assert_eq!(
        test_peer.multiaddrs.len(),
        1,
        "should have on inbound address"
    );

    let sess_ctx = SessionContext::make(
        SessionId::new(1),
        make_multiaddr(9527, None),
        SessionType::Inbound,
        test_peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 1, "should have one connection");
    assert_eq!(
        test_peer.multiaddrs.len(),
        0,
        "should remove inbound address"
    );
}

#[tokio::test]
async fn should_reject_new_connection_for_same_peer_on_new_session() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let expect_sid = test_peer.session_id();

    let sess_ctx = SessionContext::make(
        SessionId::new(99),
        test_peer.multiaddrs.all_raw().pop().expect("get multiaddr"),
        SessionType::Outbound,
        test_peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 1, "should not increase conn count");
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
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let inner = mgr.core_inner();
    let test_peer = remote_peers.first().expect("get first peer");
    inner.remove_session(test_peer.session_id());

    let sess_ctx = SessionContext::make(
        SessionId::new(99),
        test_peer.multiaddrs.all_raw().pop().expect("get multiaddr"),
        SessionType::Outbound,
        test_peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    assert_eq!(inner.connected(), 1, "should not increase conn count");
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
    let _remote_peers = make_sessions(&mut mgr, 10, 7000).await;

    let remote_pubkey = make_pubkey();
    let remote_addr = make_multiaddr(2077, Some(remote_pubkey.peer_id()));

    let sess_ctx = SessionContext::make(
        SessionId::new(99),
        remote_addr,
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
    assert_eq!(inner.connected(), 10, "should not increase conn count");

    let conn_event = conn_rx.next().await.expect("should have disconnect event");
    match conn_event {
        ConnectionEvent::Disconnect(sid) => assert_eq!(sid, 99.into(), "should be new session id"),
        _ => panic!("should be disconnect event"),
    }
}

#[tokio::test]
async fn should_remove_connecting_even_if_session_is_reject_due_to_reach_max_connections_on_new_session(
) {
    let (mut mgr, mut conn_rx) = make_manager(0, 5); // set max to 5
    let _remote_peers = make_sessions(&mut mgr, 5, 7000).await;

    let test_peer = make_peer(2020);
    let inner = mgr.core_inner();
    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));
    assert_eq!(mgr.connecting().len(), 1, "should have one attempt");

    let sess_ctx = SessionContext::make(
        SessionId::new(99),
        test_peer.multiaddrs.all_raw().pop().expect("multiaddr"),
        SessionType::Outbound,
        test_peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    assert_eq!(inner.connected(), 5, "should not increase conn count");
    assert_eq!(
        mgr.connecting().len(),
        0,
        "should remove connecting attempt"
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
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    assert_eq!(
        test_peer.retry.count(),
        0,
        "should reset retry after connect"
    );
    // Set connected at to older timestamp to increase peer alive
    test_peer.set_connected_at(time::now() - SHORT_ALIVE_SESSION - 1);

    let session_closed = PeerManagerEvent::SessionClosed {
        pid: test_peer.owned_id(),
        sid: test_peer.session_id(),
    };
    mgr.poll_event(session_closed).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 0, "shoulld have zero connected");
    assert_eq!(inner.share_sessions().len(), 0, "should have no session");
    assert_eq!(
        mgr.connecting().len(),
        1,
        "should have one connecting attempt"
    );
    assert_eq!(
        test_peer.connectedness(),
        Connectedness::Connecting,
        "should set peer connectednes to Connecting since we have't reach max connection"
    );
    assert_eq!(test_peer.retry.count(), 0, "should keep retry to 0");
}

#[tokio::test]
async fn should_increase_retry_for_short_alive_session_on_session_closed() {
    let (mut mgr, _conn_rx) = make_manager(2, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    assert_eq!(
        test_peer.retry.count(),
        0,
        "should reset retry after connect"
    );

    let session_closed = PeerManagerEvent::SessionClosed {
        pid: test_peer.owned_id(),
        sid: test_peer.session_id(),
    };
    mgr.poll_event(session_closed).await;

    let inner = mgr.core_inner();
    assert_eq!(
        inner.connected(),
        0,
        "should have no session because of retry"
    );
    assert_eq!(inner.share_sessions().len(), 0, "should have no session");
    assert_eq!(test_peer.connectedness(), Connectedness::CanConnect);
    assert!(
        test_peer.retry.eta() > REPEATED_CONNECTION_TIMEOUT,
        "should increase retry count enough to cover repeated connection timeout"
    );
}

#[tokio::test]
async fn should_inc_peer_multiaddr_failure_count_for_io_error_on_connect_failed() {
    let (mut mgr, _conn_rx) = make_manager(1, 20);

    let inner = mgr.core_inner();
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));

    let connect_failed = PeerManagerEvent::ConnectFailed {
        addr: (*test_multiaddr).to_owned(),
        kind: ConnectionErrorKind::Io(std::io::ErrorKind::Other.into()),
    };
    mgr.poll_event(connect_failed).await;

    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(1),
        "should increase failure count"
    );
}

#[tokio::test]
async fn should_inc_peer_multiaddr_failure_count_for_dns_error_on_connect_failed() {
    let (mut mgr, _conn_rx) = make_manager(1, 20);

    let inner = mgr.core_inner();
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));

    let connect_failed = PeerManagerEvent::ConnectFailed {
        addr: (*test_multiaddr).to_owned(),
        kind: ConnectionErrorKind::DNSResolver(Box::new(std::io::Error::from(
            std::io::ErrorKind::Other,
        )) as Box<dyn std::error::Error + Send>),
    };
    mgr.poll_event(connect_failed).await;

    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(1),
        "should increase failure count"
    );
}

#[tokio::test]
async fn should_give_up_peer_multiaddr_if_peer_id_not_match_on_connect_failed() {
    let (mut mgr, _conn_rx) = make_manager(1, 20);

    let inner = mgr.core_inner();
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");
    assert_eq!(
        test_peer.multiaddrs.connectable_len(),
        1,
        "should have one connectable multiaddr"
    );

    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));

    let connect_failed = PeerManagerEvent::ConnectFailed {
        addr: (*test_multiaddr).to_owned(),
        kind: ConnectionErrorKind::PeerIdNotMatch,
    };
    mgr.poll_event(connect_failed).await;

    assert_eq!(
        test_peer.multiaddrs.connectable_len(),
        0,
        "should not have any connectable multiaddr"
    );
}

#[tokio::test]
async fn should_give_up_peer_itself_if_secio_handshake_error_on_connect_failed() {
    let (mut mgr, _conn_rx) = make_manager(1, 20);

    let inner = mgr.core_inner();
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));

    let connect_failed = PeerManagerEvent::ConnectFailed {
        addr: (*test_multiaddr).to_owned(),
        kind: ConnectionErrorKind::SecioHandshake(Box::new(std::io::Error::from(
            std::io::ErrorKind::Other,
        )) as Box<dyn std::error::Error + Send>),
    };
    mgr.poll_event(connect_failed).await;

    assert_eq!(test_peer.connectedness(), Connectedness::Unconnectable);
}

#[tokio::test]
async fn should_give_up_peer_itself_if_protocol_handle_error_on_connect_failed() {
    let (mut mgr, _conn_rx) = make_manager(1, 20);

    let inner = mgr.core_inner();
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));

    let connect_failed = PeerManagerEvent::ConnectFailed {
        addr: (*test_multiaddr).to_owned(),
        kind: ConnectionErrorKind::ProtocolHandle,
    };
    mgr.poll_event(connect_failed).await;

    assert_eq!(test_peer.connectedness(), Connectedness::Unconnectable);
}

#[tokio::test]
async fn should_increase_peer_retry_if_all_multiaddrs_failed_on_conect_failed() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);

    let inner = mgr.core_inner();
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));

    let connect_failed = PeerManagerEvent::ConnectFailed {
        addr: (*test_multiaddr).to_owned(),
        kind: ConnectionErrorKind::Io(std::io::ErrorKind::Other.into()),
    };
    mgr.poll_event(connect_failed).await;

    assert_eq!(mgr.connecting().len(), 0, "should not have any connecting");
    assert_eq!(test_peer.retry.count(), 1, "should have 1 retry");
    assert_eq!(test_peer.connectedness(), Connectedness::CanConnect);
}

#[tokio::test]
async fn should_give_up_peer_if_run_out_retry_on_connect_failed() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);

    let inner = mgr.core_inner();
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));

    test_peer.retry.set(MAX_RETRY_COUNT);
    let connect_failed = PeerManagerEvent::ConnectFailed {
        addr: (*test_multiaddr).to_owned(),
        kind: ConnectionErrorKind::Io(std::io::ErrorKind::Other.into()),
    };
    mgr.poll_event(connect_failed).await;

    assert_eq!(mgr.connecting().len(), 0, "should not have any connecting");
    assert_eq!(
        test_peer.retry.count(),
        MAX_RETRY_COUNT + 1,
        "should exceed max retry"
    );
    assert_eq!(test_peer.connectedness(), Connectedness::Unconnectable);
}

#[tokio::test]
async fn should_return_early_if_we_already_give_up_peer_on_connect_failed() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);

    let inner = mgr.core_inner();
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));

    let connect_failed = PeerManagerEvent::ConnectFailed {
        addr: (*test_multiaddr).to_owned(),
        kind: ConnectionErrorKind::ProtocolHandle,
    };
    mgr.poll_event(connect_failed).await;

    assert_eq!(mgr.connecting().len(), 0, "should not have any connecting");
    assert_eq!(test_peer.connectedness(), Connectedness::Unconnectable);
    assert_eq!(test_peer.retry.count(), 0, "should not touch peer retry");
}

#[tokio::test]
async fn should_wait_for_other_connecting_multiaddrs_if_we_dont_give_up_peer_on_connect_failed() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);

    let inner = mgr.core_inner();
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");
    test_peer
        .multiaddrs
        .insert(vec![make_peer_multiaddr(2020, test_peer.owned_id())]);
    assert_eq!(
        test_peer.multiaddrs.connectable_len(),
        2,
        "should have two connectable multiaddrs"
    );

    inner.add_peer(test_peer.clone());
    mgr.connecting_mut()
        .insert(ConnectingAttempt::new(test_peer.clone()));

    let attempt = mgr.connecting().iter().next().expect("attempt");
    assert_eq!(
        attempt.multiaddrs(),
        2,
        "should still have two connecting multiaddrs"
    );

    let connect_failed = PeerManagerEvent::ConnectFailed {
        addr: (*test_multiaddr).to_owned(),
        kind: ConnectionErrorKind::Io(std::io::ErrorKind::Other.into()),
    };
    mgr.poll_event(connect_failed).await;

    assert_eq!(mgr.connecting().len(), 1, "should not have any connecting");

    let attempt = mgr.connecting().iter().next().expect("attempt");
    assert_eq!(
        attempt.multiaddrs(),
        1,
        "should still have one connecting multiaddr"
    );
}

#[tokio::test]
async fn should_ensure_disconnect_session_on_session_failed() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let expect_sid = test_peer.session_id();
    let session_failed = PeerManagerEvent::SessionFailed {
        sid:  expect_sid,
        kind: SessionErrorKind::Io(std::io::ErrorKind::Other.into()),
    };
    mgr.poll_event(session_failed).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.share_sessions().len(), 0, "should disconnect session");
    assert_eq!(inner.connected(), 0, "should disconnect session");
    assert_eq!(
        test_peer.connectedness(),
        Connectedness::CanConnect,
        "should disconnect peer"
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
async fn should_increase_retry_for_io_error_on_session_failed() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let expect_sid = test_peer.session_id();
    let session_failed = PeerManagerEvent::SessionFailed {
        sid:  expect_sid,
        kind: SessionErrorKind::Io(std::io::ErrorKind::Other.into()),
    };
    mgr.poll_event(session_failed).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 0, "should disconnect session");
    assert_eq!(test_peer.retry.count(), 1, "should increase onen retry");
}

#[tokio::test]
async fn should_give_up_peer_for_protocol_error_on_session_failed() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let expect_sid = test_peer.session_id();
    let session_failed = PeerManagerEvent::SessionFailed {
        sid:  expect_sid,
        kind: SessionErrorKind::Protocol {
            identity: None,
            cause:    None,
        },
    };
    mgr.poll_event(session_failed).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 0, "should disconnect session");
    assert_eq!(
        test_peer.connectedness(),
        Connectedness::Unconnectable,
        "should give up peer"
    );
}

#[tokio::test]
async fn should_give_up_peer_for_unexpected_error_on_session_failed() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let expect_sid = test_peer.session_id();
    let session_failed = PeerManagerEvent::SessionFailed {
        sid:  expect_sid,
        kind: SessionErrorKind::Unexpected(
            Box::new(std::io::Error::from(std::io::ErrorKind::Other))
                as Box<dyn std::error::Error + Send>,
        ),
    };
    mgr.poll_event(session_failed).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 0, "should disconnect session");
    assert_eq!(
        test_peer.connectedness(),
        Connectedness::Unconnectable,
        "should give up peer"
    );
}

#[tokio::test]
async fn should_update_peer_alive_on_peer_alive() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let old_alive = test_peer.alive();

    // Set connected at to older timestamp to increase peer alive
    test_peer.set_connected_at(time::now() - SHORT_ALIVE_SESSION - 1);

    let peer_alive = PeerManagerEvent::PeerAlive {
        pid: test_peer.owned_id(),
    };
    mgr.poll_event(peer_alive).await;

    assert_eq!(
        test_peer.alive(),
        old_alive + SHORT_ALIVE_SESSION + 1,
        "should update peer alive"
    );
}

#[tokio::test]
async fn should_reset_peer_retry_on_peer_alive() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    assert_eq!(test_peer.retry.count(), 0, "should have 0 retry");

    test_peer.retry.inc();
    assert_eq!(test_peer.retry.count(), 1, "should now have 1 retry");

    let peer_alive = PeerManagerEvent::PeerAlive {
        pid: test_peer.owned_id(),
    };
    mgr.poll_event(peer_alive).await;

    assert_eq!(test_peer.retry.count(), 0, "should reset retry");
}

#[tokio::test]
async fn should_disconnect_peer_on_misbehave() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let expect_sid = test_peer.session_id();
    let peer_misbehave = PeerManagerEvent::Misbehave {
        pid:  test_peer.owned_id(),
        kind: MisbehaviorKind::PingTimeout,
    };
    mgr.poll_event(peer_misbehave).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 0, "should disconnect session");
    assert_eq!(inner.share_sessions().len(), 0, "should disconnect session");

    let conn_event = conn_rx.next().await.expect("should have disconnect event");
    match conn_event {
        ConnectionEvent::Disconnect(sid) => {
            assert_eq!(sid, expect_sid, "should disconnect session")
        }
        _ => panic!("should be disconnect event"),
    }
}

#[tokio::test]
async fn should_increase_retry_for_ping_timeout_on_misbehave() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let peer_misbehave = PeerManagerEvent::Misbehave {
        pid:  test_peer.owned_id(),
        kind: MisbehaviorKind::PingTimeout,
    };
    mgr.poll_event(peer_misbehave).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 0, "should disconnect session");
    assert_eq!(test_peer.retry.count(), 1, "should increase retry");
}

#[tokio::test]
async fn should_give_up_peer_for_ping_unexpect_on_misbehave() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let peer_misbehave = PeerManagerEvent::Misbehave {
        pid:  test_peer.owned_id(),
        kind: MisbehaviorKind::PingUnexpect,
    };
    mgr.poll_event(peer_misbehave).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 0, "should disconnect session");
    assert_eq!(
        test_peer.connectedness(),
        Connectedness::Unconnectable,
        "should give up peer"
    );
}

#[tokio::test]
async fn should_give_up_peer_for_discovery_on_misbehave() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let peer_misbehave = PeerManagerEvent::Misbehave {
        pid:  test_peer.owned_id(),
        kind: MisbehaviorKind::Discovery,
    };
    mgr.poll_event(peer_misbehave).await;

    let inner = mgr.core_inner();
    assert_eq!(inner.connected(), 0, "should disconnect session");
    assert_eq!(
        test_peer.connectedness(),
        Connectedness::Unconnectable,
        "should give up peer"
    );
}

#[tokio::test]
async fn should_mark_session_blocked_on_session_blocked() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;

    let test_peer = remote_peers.first().expect("get first peer");
    let sess_ctx = SessionContext::make(
        test_peer.session_id(),
        test_peer.multiaddrs.all_raw().pop().expect("get multiaddr"),
        SessionType::Outbound,
        test_peer.owned_pubkey().expect("pubkey"),
    );
    let session_blocked = PeerManagerEvent::SessionBlocked {
        ctx: sess_ctx.arced(),
    };
    mgr.poll_event(session_blocked).await;

    let inner = mgr.core_inner();
    let session = inner
        .session(test_peer.session_id())
        .expect("should have a session");
    assert!(session.is_blocked(), "should be blocked");
}

#[tokio::test]
async fn should_try_all_peer_multiaddrs_on_connect_peers_now() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let peers = (0..10)
        .map(|port| {
            // Every peer has two multiaddrs
            let p = make_peer(port + 7000);
            p.multiaddrs
                .insert(vec![make_peer_multiaddr(port + 8000, p.owned_id())]);
            p
        })
        .collect::<Vec<_>>();

    let inner = mgr.core_inner();
    for peer in peers.iter() {
        inner.add_peer(peer.clone());
    }

    assert_eq!(
        mgr.connecting().len(),
        0,
        "should have 0 connecting attempt"
    );

    let connect_peers = PeerManagerEvent::ConnectPeersNow {
        pids: peers.iter().map(|p| p.owned_id()).collect(),
    };
    mgr.poll_event(connect_peers).await;

    assert_eq!(
        mgr.connecting().len(),
        10,
        "should have all peer in connecting attempt"
    );

    let conn_event = conn_rx.next().await.expect("should have connect event");
    let multiaddrs_in_event = match conn_event {
        ConnectionEvent::Connect { addrs, .. } => addrs,
        _ => panic!("should be connect event"),
    };

    let expect_multiaddrs = peers
        .into_iter()
        .map(|p| p.multiaddrs.all_raw())
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
async fn should_skip_peers_not_in_can_connect_or_not_connected_connectedness_on_connect_peers_now()
{
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let peer_in_connected = make_peer(2020);
    let peer_in_unconnectable = make_peer(2059);

    peer_in_unconnectable.set_connectedness(Connectedness::Unconnectable);
    peer_in_connected.set_connectedness(Connectedness::Connected);

    let inner = mgr.core_inner();
    inner.add_peer(peer_in_connected.clone());
    inner.add_peer(peer_in_unconnectable.clone());

    let connect_peers = PeerManagerEvent::ConnectPeersNow {
        pids: vec![
            peer_in_unconnectable.owned_id(),
            peer_in_connected.owned_id(),
        ],
    };
    mgr.poll_event(connect_peers).await;

    match conn_rx.try_next() {
        Err(_) => (), // Err means channel is empty, it's expected
        _ => panic!("should not have any connection event"),
    }
}

#[tokio::test]
async fn should_connect_peers_even_if_they_are_not_retry_ready_on_connect_peers_now() {
    let (mut mgr, mut conn_rx) = make_manager(0, 20);
    let not_ready_peer = make_peer(2077);
    not_ready_peer.retry.inc();

    let inner = mgr.core_inner();
    inner.add_peer(not_ready_peer.clone());

    let connect_peers = PeerManagerEvent::ConnectPeersNow {
        pids: vec![not_ready_peer.owned_id()],
    };
    mgr.poll_event(connect_peers).await;

    let conn_event = conn_rx.next().await.expect("should have connect event");
    let multiaddrs_in_event = match conn_event {
        ConnectionEvent::Connect { addrs, .. } => addrs,
        _ => panic!("should be connect event"),
    };

    let expect_multiaddrs = not_ready_peer.multiaddrs.all_raw();
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
async fn should_insert_peers_on_discover_multi_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let peers = (0..10)
        .map(|port| make_peer(port + 7000))
        .collect::<Vec<_>>();

    let peer_ids = peers
        .clone()
        .into_iter()
        .map(|p| p.owned_id())
        .collect::<Vec<_>>();
    let test_multiaddrs = peers
        .into_iter()
        .map(|p| p.multiaddrs.all_raw().pop().expect("multiaddr"))
        .collect::<Vec<_>>();

    let discover_multi_addrs = PeerManagerEvent::DiscoverMultiAddrs {
        addrs: test_multiaddrs,
    };
    mgr.poll_event(discover_multi_addrs).await;

    let inner = mgr.core_inner();
    assert!(
        !peer_ids.iter().any(|pid| !inner.contains(pid)),
        "should insert all discovered peers"
    );
}

#[tokio::test]
async fn should_not_reset_exist_multiaddr_failure_count_on_discover_multi_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let test_peer = make_peer(2077);
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    test_peer.multiaddrs.inc_failure(&test_multiaddr);
    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(1),
        "should have one failure"
    );

    let discover_multi_addrs = PeerManagerEvent::DiscoverMultiAddrs {
        addrs: vec![test_multiaddr.clone().into()],
    };
    mgr.poll_event(discover_multi_addrs).await;

    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(1),
        "should have one failure"
    );
}

#[tokio::test]
async fn should_skip_our_listen_multiaddrs_on_discover_multi_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let self_id = mgr.inner.peer_id.to_owned();

    let inner = mgr.core_inner();
    let listen_multiaddr = make_peer_multiaddr(2020, self_id.clone());

    inner.add_listen(listen_multiaddr.clone());
    assert!(
        inner.listen().contains(&listen_multiaddr),
        "should contains listen addr"
    );

    let discover_multi_addrs = PeerManagerEvent::DiscoverMultiAddrs {
        addrs: vec![make_multiaddr(2020, Some(self_id.clone()))],
    };
    mgr.poll_event(discover_multi_addrs).await;

    assert!(!inner.contains(&self_id), "should not add our self peer id");
}

#[tokio::test]
async fn should_add_multiaddrs_to_peer_on_identified_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;
    let test_peer = remote_peers.first().expect("get first");
    let old_multiaddrs_len = test_peer.multiaddrs.len();

    let test_multiaddrs: Vec<_> = (0..2)
        .map(|port| make_multiaddr(port + 9000, Some(test_peer.owned_id())))
        .collect();

    let identified_addrs = PeerManagerEvent::IdentifiedAddrs {
        pid:   test_peer.owned_id(),
        addrs: test_multiaddrs.clone(),
    };
    mgr.poll_event(identified_addrs).await;

    assert_eq!(
        test_peer.multiaddrs.len(),
        old_multiaddrs_len + 2,
        "should have correct multiaddrs len"
    );
    assert!(
        !test_multiaddrs
            .iter()
            .any(|ma| !test_peer.multiaddrs.all_raw().contains(ma)),
        "should add all multiaddrs to peer"
    );
}

#[tokio::test]
async fn should_push_id_to_multiaddrs_if_not_included_on_identified_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;
    let test_peer = remote_peers.first().expect("get first");
    let test_multiaddr = make_multiaddr(2077, None);

    let identified_addrs = PeerManagerEvent::IdentifiedAddrs {
        pid:   test_peer.owned_id(),
        addrs: vec![test_multiaddr.clone()],
    };
    mgr.poll_event(identified_addrs).await;

    assert!(
        !test_peer.multiaddrs.all_raw().contains(&test_multiaddr),
        "should not contain multiaddr without id included"
    );

    let with_id = make_peer_multiaddr(2077, test_peer.owned_id());
    assert!(
        test_peer.multiaddrs.contains(&with_id),
        "should push id to multiaddr when add it to peer"
    );
}

#[tokio::test]
async fn should_not_reset_exist_multiaddr_failure_count_on_identified_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;
    let test_peer = remote_peers.first().expect("get first");
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    test_peer.multiaddrs.inc_failure(&test_multiaddr);
    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(1),
        "should have one failure"
    );

    let identified_addrs = PeerManagerEvent::IdentifiedAddrs {
        pid:   test_peer.owned_id(),
        addrs: vec![test_multiaddr.clone().into()],
    };
    mgr.poll_event(identified_addrs).await;

    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(1),
        "should have one failure"
    );
}

#[tokio::test]
async fn should_reset_peer_failure_for_outbound_multiaddr_on_repeated_connection() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;
    let test_peer = remote_peers.first().expect("get first");
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    test_peer.multiaddrs.inc_failure(&test_multiaddr);
    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(1),
        "should have one failure"
    );

    let repeated_connection = PeerManagerEvent::RepeatedConnection {
        ty:   ConnectionType::Outbound,
        sid:  test_peer.session_id(),
        addr: test_multiaddr.clone().into(),
    };
    mgr.poll_event(repeated_connection).await;

    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(0),
        "should have one failure"
    );
}

#[tokio::test]
async fn should_remove_inbound_multiaddr_on_repeated_connection() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;
    let test_peer = remote_peers.first().expect("get first");

    let test_multiaddr = make_peer_multiaddr(2077, test_peer.owned_id());
    test_peer.multiaddrs.insert(vec![test_multiaddr.clone()]);

    let repeated_connection = PeerManagerEvent::RepeatedConnection {
        ty:   ConnectionType::Inbound,
        sid:  test_peer.session_id(),
        addr: test_multiaddr.clone().into(),
    };
    mgr.poll_event(repeated_connection).await;

    assert!(
        !test_peer.multiaddrs.contains(&test_multiaddr),
        "should remove inbound multiaddr"
    );
}

#[tokio::test]
async fn should_enforce_id_if_not_included_on_repeated_connection() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let remote_peers = make_sessions(&mut mgr, 1, 5000).await;
    let test_peer = remote_peers.first().expect("get first");
    let test_multiaddr = test_peer.multiaddrs.all().pop().expect("multiaddr");

    test_peer.multiaddrs.inc_failure(&test_multiaddr);
    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(1),
        "should have one failure"
    );

    let repeated_connection = PeerManagerEvent::RepeatedConnection {
        ty:   ConnectionType::Outbound,
        sid:  test_peer.session_id(),
        addr: test_multiaddr.clone().into(),
    };
    mgr.poll_event(repeated_connection).await;

    assert_eq!(
        test_peer.multiaddrs.failure(&test_multiaddr),
        Some(0),
        "should have one failure"
    );
}

#[tokio::test]
async fn should_add_new_listen_on_add_new_listen_addr() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let self_id = mgr.inner.peer_id.to_owned();

    let inner = mgr.core_inner();
    let listen_multiaddr = make_peer_multiaddr(2020, self_id.clone());
    inner.add_listen(listen_multiaddr.clone());
    assert!(!inner.listen().is_empty(), "should have listen address");

    let test_multiaddr = make_multiaddr(2077, Some(self_id));
    assert!(test_multiaddr != *listen_multiaddr);

    let add_listen_addr = PeerManagerEvent::AddNewListenAddr {
        addr: test_multiaddr.clone(),
    };
    mgr.poll_event(add_listen_addr).await;

    assert_eq!(inner.listen().len(), 2, "should have 2 listen addrs");
    assert!(
        inner.listen().contains(&test_multiaddr),
        "should add new listen multiaddr"
    );
}

#[tokio::test]
async fn should_push_id_to_listen_multiaddr_if_not_included_on_add_new_listen_addr() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let self_id = mgr.inner.peer_id.to_owned();

    let inner = mgr.core_inner();
    let test_multiaddr = make_multiaddr(2077, None);
    assert!(inner.listen().is_empty(), "should not have any listen addr");

    let add_listen_addr = PeerManagerEvent::AddNewListenAddr {
        addr: test_multiaddr.clone(),
    };
    mgr.poll_event(add_listen_addr).await;

    let with_id = make_multiaddr(2077, Some(self_id));
    assert_eq!(inner.listen().len(), 1, "should have one listen addr");
    assert!(
        inner.listen().contains(&with_id),
        "should add new listen multiaddr"
    );
}

#[tokio::test]
async fn should_remove_listen_on_remove_listen_addr() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let self_id = mgr.inner.peer_id.to_owned();

    let inner = mgr.core_inner();
    let listen_multiaddr = make_peer_multiaddr(2020, self_id.clone());

    inner.add_listen(listen_multiaddr.clone());
    assert!(
        inner.listen().contains(&listen_multiaddr),
        "should contains listen addr"
    );

    let remove_listen_addr = PeerManagerEvent::RemoveListenAddr {
        addr: make_multiaddr(2020, Some(self_id)),
    };
    mgr.poll_event(remove_listen_addr).await;

    assert_eq!(inner.listen().len(), 0, "should have 0 listen addrs");
}

#[tokio::test]
async fn should_remove_listen_even_if_no_peer_id_included_on_remove_listen_addr() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);
    let self_id = mgr.inner.peer_id.to_owned();

    let inner = mgr.core_inner();
    let listen_multiaddr = make_peer_multiaddr(2020, self_id.clone());

    inner.add_listen(listen_multiaddr.clone());
    assert!(
        inner.listen().contains(&listen_multiaddr),
        "should contains listen addr"
    );

    let remove_listen_addr = PeerManagerEvent::RemoveListenAddr {
        addr: make_multiaddr(2020, None),
    };
    mgr.poll_event(remove_listen_addr).await;

    assert_eq!(inner.listen().len(), 0, "should have 0 listen addrs");
}

#[tokio::test]
async fn should_always_include_our_listen_addrs_in_return_from_manager_handle_random_addrs() {
    let (mgr, _conn_rx) = make_manager(0, 20);
    let self_id = mgr.inner.peer_id.to_owned();

    let inner = mgr.core_inner();
    let listen_multiaddrs = (0..5)
        .map(|port| make_peer_multiaddr(port + 9000, self_id.clone()))
        .collect::<Vec<_>>();

    for ma in listen_multiaddrs.iter() {
        inner.add_listen(ma.clone());
    }

    let handle = mgr.inner.handle();
    let addrs = handle.random_addrs(100);

    assert!(
        !listen_multiaddrs.iter().any(|lma| !addrs.contains(&*lma)),
        "should include our listen addresses"
    );
}

#[tokio::test]
async fn should_whitelist_peer_chain_addrs_on_whitelist_peers_by_chain_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 20);

    let peers = (0..5)
        .map(|port| make_peer(port + 9000))
        .collect::<Vec<_>>();
    let chain_addrs = peers
        .into_iter()
        .map(|p| p.owned_chain_addr().expect("chain addr"))
        .collect::<Vec<_>>();

    let inner = mgr.core_inner();
    assert!(inner.whitelist().is_empty(), "should have empty whitelist");

    let whitelist_peers_by_chain_addrs = PeerManagerEvent::WhitelistPeersByChainAddr {
        chain_addrs: chain_addrs.clone(),
    };
    mgr.poll_event(whitelist_peers_by_chain_addrs).await;

    let whitelist = inner.whitelist();
    assert_eq!(
        whitelist.len(),
        chain_addrs.len(),
        "should have chain addrs"
    );
    assert!(
        !chain_addrs.into_iter().any(|ca| !whitelist.contains(&ca)),
        "should add all chain addrs"
    );
}

#[tokio::test]
async fn should_allow_whitelisted_peer_session_even_if_we_reach_max_connections_on_new_session() {
    let (mut mgr, _conn_rx) = make_manager(0, 10);
    let _remote_peers = make_sessions(&mut mgr, 10, 5000).await;

    let whitelisted_peer = make_peer(2077);
    let peer = make_peer(2019);

    let inner = mgr.core_inner();
    inner.whitelist_peers_by_chain_addr(vec![whitelisted_peer
        .owned_chain_addr()
        .expect("chain addr")]);

    assert_eq!(inner.whitelist().len(), 1, "should have one whitelisted");
    assert_eq!(inner.connected(), 10, "should have 10 connections");

    // First no whitelisted one
    let sess_ctx = SessionContext::make(
        SessionId::new(233),
        peer.multiaddrs.all_raw().pop().expect("peer multiaddr"),
        SessionType::Inbound,
        peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    peer.owned_id(),
        pubkey: peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    assert_eq!(inner.connected(), 10, "should remain 10 connections");

    // Now whitelistd one
    let sess_ctx = SessionContext::make(
        SessionId::new(666),
        whitelisted_peer
            .multiaddrs
            .all_raw()
            .pop()
            .expect("peer multiaddr"),
        SessionType::Inbound,
        whitelisted_peer.owned_pubkey().expect("whitelist pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    whitelisted_peer.owned_id(),
        pubkey: whitelisted_peer.owned_pubkey().expect("whitelist pubkey"),
        ctx:    sess_ctx.arced(),
    };
    mgr.poll_event(new_session).await;

    assert_eq!(inner.connected(), 11, "should remain 11 connections");
    let session = inner.session(666.into()).expect("should have session");
    assert_eq!(
        session.peer.id, whitelisted_peer.id,
        "should be whitelisted peer"
    );
}

#[tokio::test]
async fn should_refresh_whitelist_on_whitelist_peers_by_chain_addrs() {
    let (mut mgr, _conn_rx) = make_manager(0, 10);
    let peer = make_peer(2077);

    let inner = mgr.core_inner();
    inner.whitelist_peers_by_chain_addr(vec![peer.owned_chain_addr().expect("chain addr")]);
    assert_eq!(inner.whitelist().len(), 1, "should have one whitelisted");

    let peer_in_list = inner
        .whitelist()
        .iter()
        .next()
        .expect("should have one whitelist peer")
        .clone();

    // Set expire_time_at to older timestamp
    peer_in_list.set_expire_time_at(time::now() + WHITELIST_TIMEOUT - 20);
    assert!(!peer_in_list.is_expired(), "should not be expired");

    let whitelist_peers_by_chain_addrs = PeerManagerEvent::WhitelistPeersByChainAddr {
        chain_addrs: vec![peer.owned_chain_addr().expect("chain addr")],
    };
    mgr.poll_event(whitelist_peers_by_chain_addrs).await;

    assert_eq!(
        peer_in_list.expire_time(),
        TestExpireTime::At(time::now() + WHITELIST_TIMEOUT),
        "should be refreshed"
    );
}

#[tokio::test]
async fn should_remove_expired_peers_in_whitelist() {
    let (mut mgr, _conn_rx) = make_manager(0, 10);
    let peer = make_peer(2077);

    let inner = mgr.core_inner();
    inner.whitelist_peers_by_chain_addr(vec![peer.owned_chain_addr().expect("chain addr")]);
    assert_eq!(inner.whitelist().len(), 1, "should have one whitelisted");

    let peer_in_list = inner
        .whitelist()
        .iter()
        .next()
        .expect("should have one whitelist peer")
        .clone();
    // Set expire_time_at to older timestamp
    peer_in_list.set_expire_time_at(time::now() - WHITELIST_TIMEOUT - 1);
    assert!(peer_in_list.is_expired(), "should be expired");

    mgr.poll().await;
    assert_eq!(
        inner.whitelist().len(),
        0,
        "should remove expired peers in whitelist"
    );
}

#[tokio::test]
async fn should_never_expire_peers_from_config_whitelist() {
    let manager_pubkey = make_pubkey();
    let manager_id = manager_pubkey.peer_id();
    let bootstraps = make_bootstraps(10);
    let mut peer_dat_file = std::env::temp_dir();
    peer_dat_file.push("peer.dat");

    let test_peer = make_peer(2077);
    let test_chain_addr = test_peer
        .owned_chain_addr()
        .expect("test whitelist chain addr");

    let config = PeerManagerConfig {
        our_id: manager_id,
        pubkey: manager_pubkey,
        bootstraps,
        whitelist_by_chain_addrs: vec![test_chain_addr.clone()],
        whitelist_peers_only: false,
        max_connections: 10,
        routine_interval: Duration::from_secs(10),
        peer_dat_file,
    };

    let (conn_tx, _conn_rx) = unbounded();
    let (_mgr_tx, mgr_rx) = unbounded();
    let manager = PeerManager::new(config, mgr_rx, conn_tx);

    let inner = manager.inner();
    assert!(
        inner.whitelisted_by_chain_addr(&test_chain_addr),
        "should be whitelisted"
    );

    let peer_in_list = inner
        .whitelist()
        .iter()
        .next()
        .expect("should have one whitelist peer")
        .clone();

    assert_eq!(
        peer_in_list.expire_time(),
        TestExpireTime::Never,
        "should be refreshed"
    );
}

#[tokio::test]
async fn should_not_refresh_never_expired_peers_from_config_whitelist() {
    let manager_pubkey = make_pubkey();
    let manager_id = manager_pubkey.peer_id();
    let bootstraps = make_bootstraps(10);
    let mut peer_dat_file = std::env::temp_dir();
    peer_dat_file.push("peer.dat");

    let test_peer = make_peer(2077);
    let test_chain_addr = test_peer
        .owned_chain_addr()
        .expect("test whitelist chain addr");

    let config = PeerManagerConfig {
        our_id: manager_id,
        pubkey: manager_pubkey,
        bootstraps,
        whitelist_by_chain_addrs: vec![test_chain_addr.clone()],
        whitelist_peers_only: false,
        max_connections: 10,
        routine_interval: Duration::from_secs(10),
        peer_dat_file,
    };

    let (conn_tx, _conn_rx) = unbounded();
    let (_mgr_tx, mgr_rx) = unbounded();
    let manager = PeerManager::new(config, mgr_rx, conn_tx);

    let inner = manager.inner();
    assert!(
        inner.whitelisted_by_chain_addr(&test_chain_addr),
        "should be whitelisted"
    );

    let peer_in_list = inner
        .whitelist()
        .iter()
        .next()
        .expect("should have one whitelist peer")
        .clone();

    assert_eq!(
        peer_in_list.expire_time(),
        TestExpireTime::Never,
        "should be refreshed"
    );

    // Set expire_time_at to older timestamp
    peer_in_list.refresh_expire_time();
    assert!(!peer_in_list.is_expired(), "should not be expired");

    assert_eq!(
        peer_in_list.expire_time(),
        TestExpireTime::Never,
        "should not be refreshed"
    );
}

#[tokio::test]
async fn should_only_connect_peers_in_whitelist_if_whitelist_only_enabled() {
    let manager_pubkey = make_pubkey();
    let manager_id = manager_pubkey.peer_id();
    let mut peer_dat_file = std::env::temp_dir();
    peer_dat_file.push("peer.dat");

    let test_peer = make_peer(2077);
    let test_chain_addr = test_peer
        .owned_chain_addr()
        .expect("test whitelist chain addr");

    let another_peer = make_peer(2020);

    let config = PeerManagerConfig {
        our_id: manager_id,
        pubkey: manager_pubkey,
        bootstraps: Default::default(),
        whitelist_by_chain_addrs: vec![test_chain_addr.clone()],
        whitelist_peers_only: true,
        max_connections: 10,
        routine_interval: Duration::from_secs(10),
        peer_dat_file,
    };

    let (conn_tx, mut conn_rx) = unbounded();
    let (mgr_tx, mgr_rx) = unbounded();
    let manager = PeerManager::new(config, mgr_rx, conn_tx);

    let inner = manager.inner();
    inner.add_peer(test_peer.clone());
    inner.add_peer(another_peer);
    assert!(
        inner.whitelisted_by_chain_addr(&test_chain_addr),
        "should be whitelisted"
    );

    let mut manager = MockManager::new(manager, mgr_tx);
    manager.poll().await;

    let conn_event = conn_rx.next().await.expect("should have connect event");
    let multiaddrs_in_event = match conn_event {
        ConnectionEvent::Connect { addrs, .. } => addrs,
        _ => panic!("should be connect event"),
    };

    assert_eq!(
        multiaddrs_in_event.len(),
        1,
        "should have on multiaddr to connect"
    );

    let test_peer_multiaddr = test_peer.multiaddrs.all_raw().pop().expect("get multiaddr");
    assert_eq!(
        multiaddrs_in_event[0], test_peer_multiaddr,
        "should be peer in whitelist"
    );
}

#[tokio::test]
async fn should_only_allow_incoming_peers_in_whitelist_if_whitelist_only_enabled() {
    let manager_pubkey = make_pubkey();
    let manager_id = manager_pubkey.peer_id();
    let mut peer_dat_file = std::env::temp_dir();
    peer_dat_file.push("peer.dat");

    let test_peer = make_peer(2077);
    let test_chain_addr = test_peer
        .owned_chain_addr()
        .expect("test whitelist chain addr");

    let another_peer = make_peer(2020);

    let config = PeerManagerConfig {
        our_id: manager_id,
        pubkey: manager_pubkey,
        bootstraps: Default::default(),
        whitelist_by_chain_addrs: vec![test_chain_addr.clone()],
        whitelist_peers_only: true,
        max_connections: 10,
        routine_interval: Duration::from_secs(10),
        peer_dat_file,
    };

    let (conn_tx, _conn_rx) = unbounded();
    let (mgr_tx, mgr_rx) = unbounded();
    let manager = PeerManager::new(config, mgr_rx, conn_tx);

    let inner = manager.inner();
    inner.add_peer(test_peer.clone());
    inner.add_peer(another_peer.clone());
    assert!(
        inner.whitelisted_by_chain_addr(&test_chain_addr),
        "should be whitelisted"
    );

    let mut manager = MockManager::new(manager, mgr_tx);
    assert_eq!(inner.connected(), 0, "should have zero connections");

    // First no whitelisted one
    let sess_ctx = SessionContext::make(
        SessionId::new(233),
        another_peer
            .multiaddrs
            .all_raw()
            .pop()
            .expect("peer multiaddr"),
        SessionType::Inbound,
        another_peer.owned_pubkey().expect("pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    another_peer.owned_id(),
        pubkey: another_peer.owned_pubkey().expect("pubkey"),
        ctx:    sess_ctx.arced(),
    };
    manager.poll_event(new_session).await;

    assert_eq!(inner.connected(), 0, "should remain 0 connections");

    // Now whitelistd one
    let sess_ctx = SessionContext::make(
        SessionId::new(666),
        test_peer
            .multiaddrs
            .all_raw()
            .pop()
            .expect("peer multiaddr"),
        SessionType::Inbound,
        test_peer.owned_pubkey().expect("whitelist pubkey"),
    );
    let new_session = PeerManagerEvent::NewSession {
        pid:    test_peer.owned_id(),
        pubkey: test_peer.owned_pubkey().expect("whitelist pubkey"),
        ctx:    sess_ctx.arced(),
    };
    manager.poll_event(new_session).await;

    assert_eq!(inner.connected(), 1, "should have 1 connection");
}
