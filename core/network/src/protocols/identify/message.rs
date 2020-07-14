use prost::{EncodeError, Message};
use protocol::{Bytes, BytesMut};
use tentacle::multiaddr::Multiaddr;

use std::convert::TryFrom;

#[derive(Clone, PartialEq, Eq, Message)]
pub struct IdentifyMessage {
    #[prost(bytes, repeated, tag = "1")]
    pub listen_addrs:  Vec<Vec<u8>>,
    #[prost(bytes, tag = "2")]
    pub observed_addr: Vec<u8>,
    #[prost(string, tag = "3")]
    pub identify:      String,
}

impl IdentifyMessage {
    pub fn new(listen_addrs: Vec<Multiaddr>, observed_addr: Multiaddr, identify: String) -> Self {
        IdentifyMessage {
            listen_addrs: listen_addrs.into_iter().map(|addr| addr.to_vec()).collect(),
            observed_addr: observed_addr.to_vec(),
            identify,
        }
    }

    pub fn listen_addrs(&self) -> Vec<Multiaddr> {
        let addrs = self.listen_addrs.iter().cloned();
        let to_multiaddrs = addrs.filter_map(|bytes| Multiaddr::try_from(bytes).ok());
        to_multiaddrs.collect()
    }

    pub fn observed_addr(&self) -> Option<Multiaddr> {
        Multiaddr::try_from(self.observed_addr.clone()).ok()
    }

    pub fn into_bytes(self) -> Result<Bytes, EncodeError> {
        let mut buf = BytesMut::with_capacity(self.encoded_len());
        self.encode(&mut buf)?;

        Ok(buf.freeze())
    }
}
