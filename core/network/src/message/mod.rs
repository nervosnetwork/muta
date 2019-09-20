pub mod serde;
pub mod serde_multi;

use derive_more::Constructor;
use prost::Message;
use tentacle::{bytes::Bytes, SessionId};

use crate::{
    endpoint::Endpoint,
    error::{ErrorKind, NetworkError},
};

#[derive(Constructor)]
pub struct RawSessionMessage {
    pub(crate) sid: SessionId,
    pub(crate) msg: Bytes,
}

#[derive(Message)]
pub struct NetworkMessage {
    #[prost(string, tag = "1")]
    pub url: String,

    #[prost(bytes, tag = "2")]
    pub content: Vec<u8>,
}

impl NetworkMessage {
    pub fn new(endpoint: Endpoint, content: Bytes) -> Self {
        NetworkMessage {
            url:     endpoint.full_url().to_owned(),
            content: content.to_vec(),
        }
    }

    pub async fn encode(self) -> Result<Bytes, NetworkError> {
        let mut buf = Vec::with_capacity(self.encoded_len());

        <Self as Message>::encode(&self, &mut buf)
            .map_err(|e| ErrorKind::BadMessage(Box::new(e)))?;

        Ok(Bytes::from(buf))
    }

    pub async fn decode(bytes: Bytes) -> Result<Self, NetworkError> {
        <Self as Message>::decode(bytes).map_err(|e| ErrorKind::BadMessage(Box::new(e)).into())
    }
}

#[derive(Constructor)]
pub struct SessionMessage {
    pub(crate) sid: SessionId,
    pub(crate) msg: NetworkMessage,
}

#[cfg(test)]
mod tests {
    use protocol::types::Hash;
    use quickcheck_macros::quickcheck;
    use serde_derive::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct Hashes {
        #[serde(with = "super::serde_multi")]
        hashes: Vec<Hash>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct QHash {
        #[serde(with = "super::serde")]
        hash: Hash,
    }

    impl quickcheck::Arbitrary for QHash {
        fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> QHash {
            let msg = String::arbitrary(g);
            let hash_val = Hash::digest(msg.as_bytes().into());

            QHash { hash: hash_val }
        }
    }

    impl From<Vec<QHash>> for Hashes {
        fn from(q_hashes: Vec<QHash>) -> Hashes {
            let hashes = q_hashes
                .into_iter()
                .map(|qhash| qhash.hash)
                .collect::<Vec<_>>();

            Hashes { hashes }
        }
    }

    #[quickcheck]
    fn prop_protocol_type_serialization(hash: QHash) -> bool {
        bincode::deserialize::<QHash>(&bincode::serialize(&hash).unwrap()).is_ok()
    }

    #[quickcheck]
    fn prop_vec_protocol_type_serialization(hashes: Vec<QHash>) -> bool {
        let hashes = Hashes::from(hashes);

        bincode::deserialize::<Hashes>(&bincode::serialize(&hashes).unwrap()).is_ok()
    }
}
