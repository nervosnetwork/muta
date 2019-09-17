use std::fmt;

use bytes::Bytes;
use protocol::codec::ProtocolCodecSync;
use serde::{de, ser, Deserializer, Serializer};

pub fn serialize<T, S>(val: &T, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: ProtocolCodecSync,
{
    let bytes = val.encode_sync().map_err(ser::Error::custom)?;

    s.serialize_bytes(&bytes.to_vec())
}

struct BytesVisit;

pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: ProtocolCodecSync,
{
    let bytes = deserializer.deserialize_byte_buf(BytesVisit)?;

    <T as ProtocolCodecSync>::decode_sync(bytes).map_err(de::Error::custom)
}

impl<'de> de::Visitor<'de> for BytesVisit {
    type Value = Bytes;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("byte array")
    }

    #[inline]
    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Bytes::from(v))
    }
}
