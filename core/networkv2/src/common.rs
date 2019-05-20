use std::net::SocketAddr;

use tentacle::multiaddr::{Multiaddr, Protocol};

use core_context::{CommonValue, Context};

use crate::p2p::{Scope, SessionId};

pub fn socket_to_multiaddr(addr: &SocketAddr) -> Multiaddr {
    let mut maddr = Multiaddr::from(addr.ip());
    maddr.push(Protocol::Tcp(addr.port()));

    maddr
}

pub fn scope_from_context(ctx: Context) -> Option<Scope> {
    ctx.p2p_session_id()
        .map(|id| Scope::Single(SessionId::new(id)))
}
