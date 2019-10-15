#[macro_export]
macro_rules! impl_default_fixed_codec_for {
    ($category:ident, [$($type:ident),+]) => (
        use crate::types::$category;

        $(
            impl ProtocolFixedCodec for $category::$type {
                fn encode_fixed(&self) -> ProtocolResult<Bytes> {
                    Ok(Bytes::from(rlp::encode(self)))
                }

                fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
                    Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
                }
            }
        )+
    )
}
