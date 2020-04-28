use std::{convert::TryFrom, default::Default, mem};

use byteorder::{ByteOrder, LittleEndian};
use bytes::{Bytes, BytesMut};
use derive_more::From;
use prost::Message;

use crate::{
    codec::{CodecError, ProtocolCodecSync},
    field, impl_default_bytes_codec_for,
    types::primitive as protocol_primitive,
    ProtocolError, ProtocolResult,
};

// #####################
// Protobuf
// #####################

#[derive(Clone, Message, From)]
pub struct Hash {
    #[prost(bytes, tag = "1")]
    pub value: Vec<u8>,
}

#[derive(Clone, Message, From)]
pub struct MerkleRoot {
    #[prost(message, tag = "1")]
    pub value: Option<Hash>,
}

#[derive(Clone, Message, From)]
pub struct Address {
    #[prost(bytes, tag = "1")]
    pub value: Vec<u8>,
}

// #####################
// Conversion
// #####################

// Hash

impl From<protocol_primitive::Hash> for Hash {
    fn from(hash: protocol_primitive::Hash) -> Hash {
        let value = hash.as_bytes().to_vec();

        Hash { value }
    }
}

impl TryFrom<Hash> for protocol_primitive::Hash {
    type Error = ProtocolError;

    fn try_from(hash: Hash) -> Result<protocol_primitive::Hash, Self::Error> {
        let bytes = Bytes::from(hash.value);

        protocol_primitive::Hash::from_bytes(bytes)
    }
}

// Address
impl From<protocol_primitive::Address> for Address {
    fn from(address: protocol_primitive::Address) -> Address {
        let value = address.as_bytes().to_vec();

        Address { value }
    }
}

impl TryFrom<Address> for protocol_primitive::Address {
    type Error = ProtocolError;

    fn try_from(address: Address) -> Result<protocol_primitive::Address, Self::Error> {
        let bytes = Bytes::from(address.value);

        protocol_primitive::Address::from_bytes(bytes)
    }
}

// MerkleRoot

impl From<protocol_primitive::MerkleRoot> for MerkleRoot {
    fn from(root: protocol_primitive::MerkleRoot) -> MerkleRoot {
        let value = Some(Hash::from(root));

        MerkleRoot { value }
    }
}

impl TryFrom<MerkleRoot> for protocol_primitive::MerkleRoot {
    type Error = ProtocolError;

    fn try_from(root: MerkleRoot) -> Result<protocol_primitive::MerkleRoot, Self::Error> {
        let hash = field!(root.value, "MerkleRoot", "value")?;

        protocol_primitive::Hash::try_from(hash)
    }
}

// Chain schema
#[derive(Clone, Message)]
pub struct ChainSchema {
    #[prost(message, repeated, tag = "1")]
    pub schema: Vec<ServiceSchema>,
}

#[derive(Clone, Message)]
pub struct ServiceSchema {
    #[prost(bytes, tag = "1")]
    pub service: Vec<u8>,

    #[prost(bytes, tag = "2")]
    pub method: Vec<u8>,

    #[prost(bytes, tag = "3")]
    pub event: Vec<u8>,
}

impl From<protocol_primitive::ChainSchema> for ChainSchema {
    fn from(cs: protocol_primitive::ChainSchema) -> ChainSchema {
        let schema = cs.schema.into_iter().map(ServiceSchema::from).collect();

        ChainSchema { schema }
    }
}

impl TryFrom<ChainSchema> for protocol_primitive::ChainSchema {
    type Error = ProtocolError;

    fn try_from(cs: ChainSchema) -> Result<protocol_primitive::ChainSchema, Self::Error> {
        let schema = cs
            .schema
            .into_iter()
            .map(protocol_primitive::ServiceSchema::try_from)
            .collect::<Result<Vec<protocol_primitive::ServiceSchema>, ProtocolError>>()?;

        let cs = protocol_primitive::ChainSchema { schema };

        Ok(cs)
    }
}

impl From<protocol_primitive::ServiceSchema> for ServiceSchema {
    fn from(ss: protocol_primitive::ServiceSchema) -> ServiceSchema {
        ServiceSchema {
            service: ss.service.as_bytes().to_vec(),
            method:  ss.method.as_bytes().to_vec(),
            event:   ss.event.as_bytes().to_vec(),
        }
    }
}

impl TryFrom<ServiceSchema> for protocol_primitive::ServiceSchema {
    type Error = ProtocolError;

    fn try_from(ss: ServiceSchema) -> Result<protocol_primitive::ServiceSchema, Self::Error> {
        Ok(protocol_primitive::ServiceSchema {
            service: String::from_utf8(ss.service).map_err(CodecError::FromStringUtf8)?,
            method:  String::from_utf8(ss.method).map_err(CodecError::FromStringUtf8)?,
            event:   String::from_utf8(ss.event).map_err(CodecError::FromStringUtf8)?,
        })
    }
}

// #####################
// Codec
// #####################

// MerkleRoot and AssetID are just Hash aliases
impl_default_bytes_codec_for!(primitive, [Hash, Address, ChainSchema]);

impl ProtocolCodecSync for u64 {
    fn encode_sync(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut buf, *self);

        Ok(BytesMut::from(buf.as_ref()).freeze())
    }

    fn decode_sync(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u64(bytes.as_ref()))
    }
}

// #####################
// Util
// #####################

#[allow(dead_code)]
fn ensure_len(real: usize, expect: usize) -> Result<(), CodecError> {
    if real != expect {
        return Err(CodecError::WrongBytesLength { expect, real });
    }

    Ok(())
}
