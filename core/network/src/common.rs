use std::net::SocketAddr;

use core_context::{CommonValue, Context};

use crate::p2p::multiaddr::{Multiaddr, Protocol};
use crate::p2p::{Scope, SessionId};
use crate::Error;

// TODO: expose other protocols
pub fn socket_to_multiaddr(addr: &SocketAddr) -> Multiaddr {
    let mut maddr = Multiaddr::from(addr.ip());
    maddr.push(Protocol::Tcp(addr.port()));

    maddr
}

pub fn session_id_from_context(ctx: &Context) -> Result<SessionId, Error> {
    ctx.p2p_session_id()
        .map(SessionId::new)
        .ok_or_else(|| Error::SessionIdNotFound)
}

pub fn scope_from_context(ctx: &Context) -> Result<Scope, Error> {
    let sess_id = session_id_from_context(ctx)?;
    Ok(Scope::Single(sess_id))
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    use core_context::{Context, P2P_SESSION_ID};

    use crate::p2p::multiaddr::multiaddr;
    use crate::p2p::{Scope, SessionId};
    use crate::Error;

    use super::{scope_from_context, session_id_from_context, socket_to_multiaddr};

    #[test]
    fn test_socket_to_multiaddr_ipv4() {
        let addr = {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
            socket_to_multiaddr(&addr)
        };
        let expect = multiaddr!(Ip4([127, 0, 0, 1]), Tcp(8080u16));

        assert_eq!(addr, expect);
    }

    #[test]
    fn test_socket_to_multiaddr_ipv6() {
        let addr = {
            let addr = SocketAddr::new(
                IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff)),
                8080,
            );
            socket_to_multiaddr(&addr)
        };
        let expect = multiaddr!(Ip6([0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff]), Tcp(8080u16));

        assert_eq!(addr, expect);
    }

    #[test]
    fn test_session_id_from_context() {
        let ctx = Context::new();
        match session_id_from_context(&ctx) {
            Err(Error::SessionIdNotFound) => (),
            _ => panic!("should return Error::SessionIdNotFound"),
        }

        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);
        assert_eq!(session_id_from_context(&ctx).unwrap(), SessionId::new(1));
    }

    #[test]
    fn test_scope_from_context() {
        let ctx = Context::new();
        match scope_from_context(&ctx) {
            Err(Error::SessionIdNotFound) => (),
            _ => panic!("should return Error::SessionIdNotFound"),
        }

        let ctx = ctx.with_value(P2P_SESSION_ID, 1usize);
        let expect_scope = Scope::Single(SessionId::new(1));
        assert_eq!(scope_from_context(&ctx).unwrap(), expect_scope);
    }
}
