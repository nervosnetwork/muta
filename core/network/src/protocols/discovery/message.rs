use std::convert::TryFrom;

use prost::{Message, Oneof};
use tentacle::multiaddr::Multiaddr;

#[derive(Clone, Copy, PartialEq, Eq, Oneof)]
pub enum ListenPort {
    #[prost(uint32, tag = "1")]
    On(u32),
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct GetNodes {
    #[prost(uint32, tag = "1")]
    pub version:     u32,
    #[prost(uint32, tag = "2")]
    pub count:       u32,
    #[prost(oneof = "ListenPort", tags = "3")]
    pub listen_port: Option<ListenPort>,
}

impl GetNodes {
    pub fn listen_port(&self) -> Option<u16> {
        match self.listen_port {
            Some(ListenPort::On(port)) if port <= u16::MAX as u32 => Some(port as u16),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct Node {
    #[prost(bytes, repeated, tag = "1")]
    pub addrs: Vec<Vec<u8>>,
}

impl Node {
    pub fn addrs(self) -> Vec<Multiaddr> {
        let addrs = self.addrs.into_iter();
        let to_multiaddrs = addrs.filter_map(|bytes| Multiaddr::try_from(bytes).ok());
        to_multiaddrs.collect::<Vec<_>>()
    }

    pub fn with_addrs(addrs: Vec<Multiaddr>) -> Self {
        Node {
            addrs: addrs.into_iter().map(|addr| addr.to_vec()).collect(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct Nodes {
    #[prost(bool, tag = "1")]
    pub announce: bool,
    #[prost(message, repeated, tag = "2")]
    pub items:    Vec<Node>,
}

#[derive(Clone, PartialEq, Eq, Oneof)]
pub enum Payload {
    #[prost(message, tag = "1")]
    GetNodes(GetNodes),
    #[prost(message, tag = "2")]
    Nodes(Nodes),
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct DiscoveryMessage {
    #[prost(oneof = "Payload", tags = "1, 2")]
    pub payload: Option<Payload>,
}

impl DiscoveryMessage {
    pub fn new_get_nodes(version: u32, count: u32, listen_port: Option<u16>) -> Self {
        let listen_port = listen_port.map(|port| ListenPort::On(port as u32));

        DiscoveryMessage {
            payload: Some(Payload::GetNodes(GetNodes {
                version,
                count,
                listen_port,
            })),
        }
    }

    pub fn new_nodes(announce: bool, nodes: Vec<Vec<Multiaddr>>) -> Self {
        DiscoveryMessage {
            payload: Some(Payload::Nodes(Nodes {
                announce,
                items: nodes.into_iter().map(Node::with_addrs).collect(),
            })),
        }
    }
}

impl std::fmt::Display for DiscoveryMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            DiscoveryMessage {
                payload: Some(Payload::GetNodes(GetNodes { version, count, .. })),
            } => {
                write!(f, "Payload::GetNodes(version:{}, count:{})", version, count)?;
            }
            DiscoveryMessage {
                payload: Some(Payload::Nodes(Nodes { announce, items })),
            } => {
                write!(
                    f,
                    "Payload::Nodes(announce:{}, items.length:{})",
                    announce,
                    items.len()
                )?;
            }
            DiscoveryMessage { payload: None } => write!(f, "Empty payload")?,
        }
        Ok(())
    }
}
