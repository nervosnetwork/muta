use std::convert::TryFrom;

use prost::{EncodeError, Message};
use protocol::{Bytes, BytesMut};
use tentacle::multiaddr::Multiaddr;

#[derive(Message)]
pub struct AddressInfo {
    #[prost(bytes, repeated, tag = "1")]
    pub listen_addrs:  Vec<Vec<u8>>,
    #[prost(bytes, tag = "2")]
    pub observed_addr: Vec<u8>,
}

impl AddressInfo {
    pub fn new(listen_addrs: Vec<Multiaddr>, observed_addr: Multiaddr) -> Self {
        AddressInfo {
            listen_addrs:  listen_addrs.into_iter().map(|addr| addr.to_vec()).collect(),
            observed_addr: observed_addr.to_vec(),
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
}

#[derive(Message)]
pub struct Identity {
    #[prost(string, tag = "1")]
    pub chain_id:  String,
    #[prost(message, tag = "2")]
    pub addr_info: Option<AddressInfo>,
}

impl Identity {
    pub fn new(chain_id: String, addr_info: AddressInfo) -> Self {
        Identity {
            chain_id,
            addr_info: Some(addr_info),
        }
    }

    pub fn listen_addrs(&self) -> Vec<Multiaddr> {
        self.addr_info
            .as_ref()
            .map(|ai| ai.listen_addrs())
            .unwrap_or_else(Vec::new)
    }

    pub fn observed_addr(&self) -> Option<Multiaddr> {
        self.addr_info
            .as_ref()
            .map(|ai| ai.observed_addr())
            .flatten()
    }

    pub fn into_bytes(self) -> Result<Bytes, EncodeError> {
        let mut buf = BytesMut::with_capacity(self.encoded_len());
        self.encode(&mut buf)?;

        Ok(buf.freeze())
    }
}

#[derive(Message)]
pub struct Acknowledge {
    #[prost(message, tag = "1")]
    pub addr_info: Option<AddressInfo>,
}

impl Acknowledge {
    pub fn new(addr_info: AddressInfo) -> Self {
        Acknowledge {
            addr_info: Some(addr_info),
        }
    }

    pub fn listen_addrs(&self) -> Vec<Multiaddr> {
        self.addr_info
            .as_ref()
            .map(|ai| ai.listen_addrs())
            .unwrap_or_else(Vec::new)
    }

    pub fn observed_addr(&self) -> Option<Multiaddr> {
        self.addr_info
            .as_ref()
            .map(|ai| ai.observed_addr())
            .flatten()
    }

    pub fn into_bytes(self) -> Result<Bytes, EncodeError> {
        let mut buf = BytesMut::with_capacity(self.encoded_len());
        self.encode(&mut buf)?;

        Ok(buf.freeze())
    }
}
