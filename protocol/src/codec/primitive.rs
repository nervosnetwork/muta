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

// #####################
// Codec
// #####################

// MerkleRoot and AssetID are just Hash aliases
impl_default_bytes_codec_for!(primitive, [Hash, Address]);

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
