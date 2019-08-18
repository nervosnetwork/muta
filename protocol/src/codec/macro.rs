#[macro_export]
macro_rules! field {
    ($opt_field:expr, $type:expr, $field:expr) => {
        $opt_field.ok_or_else(|| crate::codec::CodecError::MissingField {
            r#type: $type,
            field:  $field,
        })
    };
}

#[macro_export]
macro_rules! impl_default_bytes_codec_for {
    ($category:ident, [$($type:ident),+]) => (
        use crate::types::$category;

        $(
            impl ProtocolCodecSync for $category::$type {
                fn encode_sync(&self) -> ProtocolResult<Bytes>  {
                    let ser_type = $type::from(self.clone());
                    let mut buf = Vec::with_capacity(ser_type.encoded_len());

                    ser_type.encode(&mut buf).map_err(CodecError::from)?;

                    Ok(Bytes::from(buf))
                }

                fn decode_sync(bytes: Bytes) -> ProtocolResult<Self> {
                    let ser_type = $type::decode(bytes).map_err(CodecError::from)?;

                    $category::$type::try_from(ser_type)
                }
            }
        )+
    )
}
