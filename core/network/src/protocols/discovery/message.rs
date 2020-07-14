use super::message_mol;

use bytes::{Bytes, BytesMut};
use log::debug;
use molecule::prelude::{Builder, Entity, Reader};
use tentacle::multiaddr::Multiaddr;
use tokio_util::codec::{length_delimited::LengthDelimitedCodec, Decoder, Encoder};

use std::{convert::TryFrom, io};

pub(crate) struct DiscoveryCodec {
    inner: LengthDelimitedCodec,
}

impl Default for DiscoveryCodec {
    fn default() -> DiscoveryCodec {
        DiscoveryCodec {
            inner: LengthDelimitedCodec::new(),
        }
    }
}

impl Decoder for DiscoveryCodec {
    type Error = io::Error;
    type Item = DiscoveryMessage;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.inner.decode(src) {
            Ok(Some(frame)) => {
                // TODO: more error information
                DiscoveryMessage::decode(&frame).map(Some).ok_or_else(|| {
                    debug!("deserialize error");
                    io::ErrorKind::InvalidData.into()
                })
            }
            Ok(None) => Ok(None),
            // TODO: more error information
            Err(err) => {
                debug!("decode error: {:?}", err);
                Err(io::ErrorKind::InvalidData.into())
            }
        }
    }
}

impl Encoder for DiscoveryCodec {
    type Error = io::Error;
    type Item = DiscoveryMessage;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.inner.encode(item.encode(), dst)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DiscoveryMessage {
    GetNodes {
        version:     u32,
        count:       u32,
        listen_port: Option<u16>,
    },
    Nodes(Nodes),
}

impl DiscoveryMessage {
    pub fn encode(self) -> Bytes {
        let playload = match self {
            DiscoveryMessage::GetNodes {
                version,
                count,
                listen_port,
            } => {
                let version_le = version.to_le_bytes();
                let count_le = count.to_le_bytes();
                let version = message_mol::Uint32::new_builder()
                    .nth0(version_le[0].into())
                    .nth1(version_le[1].into())
                    .nth2(version_le[2].into())
                    .nth3(version_le[3].into())
                    .build();
                let count = message_mol::Uint32::new_builder()
                    .nth0(count_le[0].into())
                    .nth1(count_le[1].into())
                    .nth2(count_le[2].into())
                    .nth3(count_le[3].into())
                    .build();
                let listen_port = message_mol::PortOpt::new_builder()
                    .set(listen_port.map(|port| {
                        let port_le = port.to_le_bytes();
                        message_mol::Uint16::new_builder()
                            .nth0(port_le[0].into())
                            .nth1(port_le[1].into())
                            .build()
                    }))
                    .build();
                let get_node = message_mol::GetNodes::new_builder()
                    .listen_port(listen_port)
                    .count(count)
                    .version(version)
                    .build();
                message_mol::DiscoveryPayload::new_builder()
                    .set(get_node)
                    .build()
            }
            DiscoveryMessage::Nodes(Nodes { announce, items }) => {
                let bool_ = if announce { 1u8 } else { 0 };
                let announce = message_mol::Bool::new_builder().set([bool_.into()]).build();
                let mut item_vec = Vec::with_capacity(items.len());
                for item in items {
                    let mut vec_addrs = Vec::with_capacity(item.addresses.len());
                    for addr in item.addresses {
                        vec_addrs.push(
                            message_mol::Bytes::new_builder()
                                .set(addr.to_vec().into_iter().map(Into::into).collect())
                                .build(),
                        )
                    }
                    let bytes_vec = message_mol::BytesVec::new_builder().set(vec_addrs).build();
                    let node = message_mol::Node::new_builder()
                        .addresses(bytes_vec)
                        .build();
                    item_vec.push(node)
                }
                let items = message_mol::NodeVec::new_builder().set(item_vec).build();
                let nodes = message_mol::Nodes::new_builder()
                    .announce(announce)
                    .items(items)
                    .build();
                message_mol::DiscoveryPayload::new_builder()
                    .set(nodes)
                    .build()
            }
        };

        message_mol::DiscoveryMessage::new_builder()
            .payload(playload)
            .build()
            .as_bytes()
    }

    #[allow(clippy::cast_ptr_alignment)]
    pub fn decode(data: &[u8]) -> Option<Self> {
        let reader = message_mol::DiscoveryMessageReader::from_compatible_slice(data).ok()?;
        match reader.payload().to_enum() {
            message_mol::DiscoveryPayloadUnionReader::GetNodes(reader) => {
                let le = reader.version().raw_data().as_ptr() as *const u32;
                let version = u32::from_le(unsafe { *le });
                let le = reader.count().raw_data().as_ptr() as *const u32;
                let count = u32::from_le(unsafe { *le });
                let listen_port = reader.listen_port().to_opt().map(|port_reader| {
                    let le = port_reader.raw_data().as_ptr() as *const u16;
                    u16::from_le(unsafe { *le })
                });
                Some(DiscoveryMessage::GetNodes {
                    version,
                    count,
                    listen_port,
                })
            }
            message_mol::DiscoveryPayloadUnionReader::Nodes(reader) => {
                let announce = match reader.announce().as_slice()[0] {
                    0 => false,
                    1 => true,
                    _ => return None,
                };
                let mut items = Vec::with_capacity(reader.items().len());
                for node_reader in reader.items().iter() {
                    let mut addresses = Vec::with_capacity(node_reader.addresses().len());
                    for address_reader in node_reader.addresses().iter() {
                        addresses
                            .push(Multiaddr::try_from(address_reader.raw_data().to_vec()).ok()?)
                    }
                    items.push(Node { addresses })
                }
                Some(DiscoveryMessage::Nodes(Nodes { announce, items }))
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Nodes {
    pub(crate) announce: bool,
    pub(crate) items:    Vec<Node>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Node {
    pub(crate) addresses: Vec<Multiaddr>,
}

impl std::fmt::Display for DiscoveryMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            DiscoveryMessage::GetNodes { version, count, .. } => {
                write!(
                    f,
                    "DiscoveryMessage::GetNodes(version:{}, count:{})",
                    version, count
                )?;
            }
            DiscoveryMessage::Nodes(Nodes { announce, items }) => {
                write!(
                    f,
                    "DiscoveryMessage::Nodes(announce:{}, items.length:{})",
                    announce,
                    items.len()
                )?;
            }
        }
        Ok(())
    }
}
