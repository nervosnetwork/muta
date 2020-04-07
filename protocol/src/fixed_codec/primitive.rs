use std::mem;

use byteorder::{ByteOrder, LittleEndian};
use bytes::{Bytes, BytesMut};

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::ProtocolResult;

impl FixedCodec for bool {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let bs = if *self {
            [1u8; mem::size_of::<u8>()]
        } else {
            [0u8; mem::size_of::<u8>()]
        };

        Ok(BytesMut::from(bs.as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        let u = *bytes.to_vec().get(0).ok_or(FixedCodecError::DecodeBool)?;

        match u {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(FixedCodecError::DecodeBool.into()),
        }
    }
}

impl FixedCodec for u8 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(BytesMut::from([*self].as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        let u = *bytes.to_vec().get(0).ok_or(FixedCodecError::DecodeUint8)?;
        Ok(u)
    }
}

impl FixedCodec for u16 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<u16>()];
        LittleEndian::write_u16(&mut buf, *self);

        Ok(BytesMut::from(buf.as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u16(bytes.as_ref()))
    }
}

impl FixedCodec for u32 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<u32>()];
        LittleEndian::write_u32(&mut buf, *self);

        Ok(BytesMut::from(buf.as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u32(bytes.as_ref()))
    }
}

impl FixedCodec for u64 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut buf, *self);

        Ok(BytesMut::from(buf.as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u64(bytes.as_ref()))
    }
}

impl FixedCodec for u128 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<u128>()];
        LittleEndian::write_u128(&mut buf, *self);

        Ok(BytesMut::from(buf.as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u128(bytes.as_ref()))
    }
}

impl FixedCodec for usize {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<usize>()];
        LittleEndian::write_uint(&mut buf, *self as u64, 64);

        Ok(BytesMut::from(buf.as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_uint(bytes.as_ref(), 64) as usize)
    }
}

impl FixedCodec for String {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(self.clone()))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        String::from_utf8(bytes.to_vec()).map_err(|e| FixedCodecError::StringUTF8(e).into())
    }
}

impl FixedCodec for Bytes {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(self.clone())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(bytes)
    }
}

impl FixedCodec for Vec<u8> {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(self.clone()))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(bytes.to_vec())
    }
}
