pub mod serde;
pub mod serde_multi;

use common_apm::muta_apm::rustracing_jaeger::span::TraceId;

use derive_more::Constructor;
use futures::channel::mpsc::UnboundedSender;
use prost::Message;
use protocol::Bytes;
use tentacle::{secio::PeerId, SessionId};

use crate::{
    common::ConnectedAddr,
    endpoint::Endpoint,
    error::{ErrorKind, NetworkError},
    event::PeerManagerEvent,
};

use std::{collections::HashMap, str::FromStr};

pub struct Headers(HashMap<String, Vec<u8>>);

impl Default for Headers {
    fn default() -> Self {
        Headers(Default::default())
    }
}

impl Headers {
    pub fn set_trace_id(&mut self, id: TraceId) {
        self.0
            .insert("trace_id".to_owned(), id.to_string().into_bytes());
    }

    pub fn set_span_id(&mut self, id: u64) {
        self.0
            .insert("span_id".to_owned(), id.to_be_bytes().to_vec());
    }
}

#[derive(Message)]
pub struct NetworkMessage {
    #[prost(map = "string, bytes", tag = "1")]
    pub headers: HashMap<String, Vec<u8>>,

    #[prost(string, tag = "2")]
    pub url: String,

    #[prost(bytes, tag = "3")]
    pub content: Vec<u8>,
}

impl NetworkMessage {
    pub fn new(endpoint: Endpoint, content: Bytes, headers: Headers) -> Self {
        NetworkMessage {
            headers: headers.0,
            url:     endpoint.full_url().to_owned(),
            content: content.to_vec(),
        }
    }

    pub fn trace_id(&self) -> Option<TraceId> {
        self.headers
            .get("trace_id")
            .map(|id| {
                String::from_utf8(id.to_owned())
                    .ok()
                    .map(|s| TraceId::from_str(&s).ok())
                    .flatten()
            })
            .flatten()
    }

    pub fn span_id(&self) -> Option<u64> {
        self.headers.get("span_id").map(|id| {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&id[..8]);
            u64::from_be_bytes(buf)
        })
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
#[non_exhaustive]
pub struct SessionMessage {
    pub(crate) sid:            SessionId,
    pub(crate) pid:            PeerId,
    pub(crate) msg:            NetworkMessage,
    pub(crate) connected_addr: Option<ConnectedAddr>,
    pub(crate) trust_tx:       UnboundedSender<PeerManagerEvent>,
}

#[cfg(test)]
mod tests {
    use protocol::{types::Hash, Bytes};
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
            let msg = Bytes::from(String::arbitrary(g));
            let hash_val = Hash::digest(msg);

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
