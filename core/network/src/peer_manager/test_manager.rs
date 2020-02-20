use super::{ArcPeer, PeerManager, PeerManagerConfig};
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

fn make_manager(
    pubkey: PublicKey,
    bootstrap_num: usize,
) -> (
    PeerManager,
    UnboundedSender<PeerManagerEvent>,
    UnboundedReceiver<ConnectionEvent>,
) {
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

    (manager, mgr_tx, conn_rx)
}

fn make_pubkey() -> PublicKey {
    let keypair = SecioKeyPair::secp256k1_generated();
    keypair.public_key()
}

struct PendingComplete<'a>(&'a mut PeerManager);

impl<'a> Future for PendingComplete<'a> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let _ = Future::poll(Pin::new(&mut self.as_mut().0), ctx);
        Poll::Ready(())
    }
}

#[tokio::test]
async fn test_new_session() {
    let our_pubkey = make_pubkey();
    let (mut mgr, mgr_tx, _conn_rx) = make_manager(our_pubkey, 2);

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
    mgr_tx
        .unbounded_send(event_new_session)
        .expect("send new session");

    PendingComplete(&mut mgr).await;
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

    let saved_session = inner.session(&1.into()).expect("should save session");
    assert_eq!(saved_session.peer.id.as_ref(), &remote_pubkey.peer_id());
    assert!(!saved_session.is_blocked());
    assert_eq!(
        saved_session.connected_addr,
        ConnectedAddr::from(&remote_addr)
    );
}
