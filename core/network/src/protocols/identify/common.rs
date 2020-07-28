use tentacle::{
    multiaddr::Multiaddr,
    utils::{is_reachable, multiaddr_to_socketaddr},
};

pub fn reachable(addr: &Multiaddr) -> bool {
    #[cfg(feature = "global_ip_only")]
    let global_ip_only = true;
    #[cfg(not(feature = "global_ip_only"))]
    let global_ip_only = false;

    multiaddr_to_socketaddr(addr)
        .map(|socket_addr| !global_ip_only || is_reachable(socket_addr.ip()))
        .unwrap_or(false)
}
