use std::io;

use tentacle::bytes::Bytes;

use crate::{error::NetworkError, traits::Compression};

#[derive(Clone)]
pub struct Snappy;

impl Compression for Snappy {
    fn compress(&self, bytes: Bytes) -> Result<Bytes, NetworkError> {
        let mut vec_bytes = Vec::with_capacity(bytes.len());

        {
            let mut writer = snap::Writer::new(&mut vec_bytes);
            let n = io::copy(&mut bytes.as_ref(), &mut writer)?;

            if n as usize != bytes.len() {
                let kind = io::ErrorKind::Other;

                return Err(io::Error::new(kind, "snappy: fail to compress").into());
            }
        }

        Ok(Bytes::from(vec_bytes))
    }

    fn decompress(&self, bytes: Bytes) -> Result<Bytes, NetworkError> {
        let mut vec_bytes = vec![];
        let mut reader = snap::Reader::new(bytes.as_ref());

        let _ = io::copy(&mut reader, &mut vec_bytes)? as usize;

        Ok(Bytes::from(vec_bytes))
    }
}
