mod array;
mod map;
mod primitive;

use bytes::Bytes;
use derive_more::{Display, From};

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

pub use array::DefaultStoreArray;
pub use map::DefaultStoreMap;
pub use primitive::{DefaultStoreBool, DefaultStoreString, DefaultStoreUint64};

pub struct FixedKeys<K: FixedCodec> {
    pub inner: Vec<K>,
}

impl<K: FixedCodec> rlp::Encodable for FixedKeys<K> {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let inner: Vec<Vec<u8>> = self
            .inner
            .iter()
            .map(|k| k.encode_fixed().expect("encode should not fail").to_vec())
            .collect();

        s.begin_list(1).append_list::<Vec<u8>, _>(&inner);
    }
}

impl<K: FixedCodec> rlp::Decodable for FixedKeys<K> {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let inner_u8: Vec<Vec<u8>> = rlp::decode_list(r.at(0)?.as_raw());

        let inner_k: Result<Vec<K>, _> = inner_u8
            .into_iter()
            .map(|v| <_>::decode_fixed(Bytes::from(v)))
            .collect();

        let inner = inner_k.map_err(|_| rlp::DecoderError::Custom("decode K from bytes fail"))?;

        Ok(FixedKeys { inner })
    }
}

impl<K: FixedCodec> FixedCodec for FixedKeys<K> {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}

#[derive(Debug, Display, From)]
pub enum StoreError {
    #[display(fmt = "the key not existed")]
    GetNone,

    #[display(fmt = "access array out of range")]
    OutRange,

    #[display(fmt = "decode error")]
    DecodeError,

    #[display(fmt = "overflow when calculating")]
    Overflow,
}

impl std::error::Error for StoreError {}

impl From<StoreError> for ProtocolError {
    fn from(err: StoreError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Binding, Box::new(err))
    }
}
