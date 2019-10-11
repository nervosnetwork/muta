use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use lazy_static::lazy_static;
use tentacle::secio::SecioKeyPair;

use core_network::{NetworkConfig, NetworkService};

pub const IP_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 137, 0, 25));
pub const BOOTSTRAP_PORT: u16 = 1337;

lazy_static! {
    pub static ref BOOTSTRAP_SECKEY: String = hex::encode("8".repeat(32));
    pub static ref BOOTSTRAP_PUBKEY: String = hex::encode(
        SecioKeyPair::secp256k1_raw_key("8".repeat(32))
            .expect("seckey")
            .to_public_key()
            .inner()
    );
    pub static ref BOOTSTRAP_ADDR: SocketAddr = SocketAddr::new(IP_ADDR, BOOTSTRAP_PORT);
}

pub fn setup_bootstrap() -> NetworkService {
    let bootstrap_conf = NetworkConfig::new()
        .secio_keypair(BOOTSTRAP_SECKEY.to_string())
        .expect("bootstrap secio keypair");

    let mut bootstrap = NetworkService::new(bootstrap_conf);

    bootstrap.listen(*BOOTSTRAP_ADDR).expect("bootstrap listen");

    bootstrap
}

pub fn setup_peer(port: u16) -> NetworkService {
    let peer_conf = NetworkConfig::new()
        .bootstraps(vec![(BOOTSTRAP_PUBKEY.to_string(), *BOOTSTRAP_ADDR)])
        .expect("peer bootstraps");

    let mut peer = NetworkService::new(peer_conf);

    peer.listen(SocketAddr::new(IP_ADDR, port))
        .expect("peer listen");

    peer
}
