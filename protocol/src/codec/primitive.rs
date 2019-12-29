use std::{convert::TryFrom, default::Default};

use bytes::Bytes;
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

// TODO: change to Amount?
#[derive(Clone, Message)]
pub struct Balance {
    #[prost(bytes, tag = "1")]
    pub value: Vec<u8>,
}

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

#[derive(Clone, Message)]
pub struct Fee {
    #[prost(message, tag = "1")]
    pub asset_id: Option<Hash>,

    #[prost(uint64, tag = "2")]
    pub cycle: u64,
}

// #####################
// Conversion
// #####################

// Balance

impl From<protocol_primitive::Balance> for Balance {
    fn from(balance: protocol_primitive::Balance) -> Balance {
        let value = balance.to_bytes_be();
        Balance { value }
    }
}

impl TryFrom<Balance> for protocol_primitive::Balance {
    type Error = ProtocolError;

    fn try_from(ser_balance: Balance) -> Result<protocol_primitive::Balance, Self::Error> {
        Ok(protocol_primitive::Balance::from_bytes_be(
            &ser_balance.value,
        ))
    }
}

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

// Fee

impl From<protocol_primitive::Fee> for Fee {
    fn from(fee: protocol_primitive::Fee) -> Fee {
        let asset_id = Hash::from(fee.asset_id);

        Fee {
            asset_id: Some(asset_id),
            cycle:    fee.cycle,
        }
    }
}

impl TryFrom<Fee> for protocol_primitive::Fee {
    type Error = ProtocolError;

    fn try_from(fee: Fee) -> Result<protocol_primitive::Fee, Self::Error> {
        let asset_id = field!(fee.asset_id, "Fee", "asset_id")?;

        let fee = protocol_primitive::Fee {
            asset_id: protocol_primitive::Hash::try_from(asset_id)?,
            cycle:    fee.cycle,
        };

        Ok(fee)
    }
}

// #####################
// Codec
// #####################

// MerkleRoot and AssetID are just Hash aliases
impl_default_bytes_codec_for!(primitive, [Balance, Hash, Fee]);

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
