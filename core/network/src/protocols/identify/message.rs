use std::convert::TryFrom;

use derive_more::Display;
use prost::{EncodeError, Message};
use protocol::{Bytes, BytesMut};
use tentacle::multiaddr::Multiaddr;

pub const MAX_LISTEN_ADDRS: usize = 10;

#[derive(Debug, Display)]
pub enum Error {
    #[display(fmt = "too many listen addrs")]
    TooManyListenAddrs,

    #[display(fmt = "no observed addrs")]
    NoObservedAddr,

    #[display(fmt = "no addr info")]
    NoAddrInfo,
}

pub trait AddressInfoMessage {
    fn validate(&self) -> Result<(), self::Error>;
    fn listen_addrs(&self) -> Vec<Multiaddr>;
    fn observed_addr(&self) -> Option<Multiaddr>;
}

impl AddressInfoMessage for Option<AddressInfo> {
    fn listen_addrs(&self) -> Vec<Multiaddr> {
        self.as_ref()
            .map(|ai| ai.listen_addrs())
            .unwrap_or_else(Vec::new)
    }

    fn observed_addr(&self) -> Option<Multiaddr> {
        self.as_ref().map(|ai| ai.observed_addr()).flatten()
    }

    fn validate(&self) -> Result<(), self::Error> {
        match self.as_ref() {
            Some(addr_info) => addr_info.validate(),
            None => Err(self::Error::NoAddrInfo),
        }
    }
}

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

    pub fn validate(&self) -> Result<(), self::Error> {
        if self.listen_addrs.len() > MAX_LISTEN_ADDRS {
            return Err(self::Error::TooManyListenAddrs);
        }

        if self.observed_addr().is_none() {
            return Err(self::Error::NoObservedAddr);
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn mock_valid() -> Self {
        let listen_addr: Multiaddr = "/ip4/47.111.169.36/tcp/2000".parse().unwrap();
        let observed_addr: Multiaddr = "/ip4/47.111.169.36/tcp/2001".parse().unwrap();

        AddressInfo {
            listen_addrs:  vec![listen_addr.to_vec()],
            observed_addr: observed_addr.to_vec(),
        }
    }

    #[cfg(test)]
    pub fn mock_invalid() -> Self {
        AddressInfo {
            listen_addrs:  vec![],
            observed_addr: b"xxx".to_vec(),
        }
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

    pub fn validate(&self) -> Result<(), self::Error> {
        self.addr_info.validate()
    }

    pub fn into_bytes(self) -> Result<Bytes, EncodeError> {
        let mut buf = BytesMut::with_capacity(self.encoded_len());
        self.encode(&mut buf)?;

        Ok(buf.freeze())
    }

    #[cfg(test)]
    pub fn mock_valid() -> Self {
        use protocol::types::Hash;

        Identity {
            chain_id:  Hash::digest(Bytes::from_static(b"hello")).as_hex(),
            addr_info: Some(AddressInfo::mock_valid()),
        }
    }

    #[cfg(test)]
    pub fn mock_invalid() -> Self {
        use protocol::types::Hash;

        let identity = Identity {
            chain_id:  Hash::digest(Bytes::from_static(b"hello")).as_hex(),
            addr_info: Some(AddressInfo::mock_invalid()),
        };
        assert!(identity.validate().is_err());

        identity
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

    pub fn validate(&self) -> Result<(), self::Error> {
        self.addr_info.validate()
    }

    pub fn into_bytes(self) -> Result<Bytes, EncodeError> {
        let mut buf = BytesMut::with_capacity(self.encoded_len());
        self.encode(&mut buf)?;

        Ok(buf.freeze())
    }

    #[cfg(test)]
    pub fn mock_valid() -> Self {
        Acknowledge {
            addr_info: Some(AddressInfo::mock_valid()),
        }
    }

    #[cfg(test)]
    pub fn mock_invalid() -> Self {
        Acknowledge {
            addr_info: Some(AddressInfo::mock_invalid()),
        }
    }
}
