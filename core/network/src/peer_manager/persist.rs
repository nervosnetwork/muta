use std::{
    fmt,
    fs::File,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

use serde::{de, ser};
use serde_derive::{Deserialize, Serialize};
use tentacle::secio::PublicKey;

use crate::{error::NetworkError, peer_manager::PeerState};

// TODO: Async support, right now, it's ok since we only load/save data once.
pub(super) trait Persistence: Send + Sync {
    fn save(&self, data: Vec<(PublicKey, PeerState)>) -> Result<(), NetworkError>;
    fn load(&self) -> Result<Vec<(PublicKey, PeerState)>, NetworkError>;
}

#[derive(Clone)]
pub(super) struct PeerPersistence {
    path: PathBuf,
}

impl PeerPersistence {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        PeerPersistence {
            path: path.as_ref().to_owned(),
        }
    }
}

impl Persistence for PeerPersistence {
    fn save(&self, peer_box: Vec<(PublicKey, PeerState)>) -> Result<(), NetworkError> {
        let mut file = File::create(&self.path)?;
        let peer_data = bincode::serialize(&PeerData::from(peer_box))?;

        file.write_all(peer_data.as_slice())?;
        Ok(())
    }

    // Load data only happen once during network service starting, sync version
    // is ok.
    fn load(&self) -> Result<Vec<(PublicKey, PeerState)>, NetworkError> {
        let file = File::open(&self.path)?;
        let mut buf_reader = BufReader::new(file);
        let mut data = Vec::new();

        buf_reader.read_to_end(&mut data)?;
        let peer_data: PeerData = bincode::deserialize(&data)?;

        Ok(peer_data.unbox())
    }
}

#[derive(Clone)]
pub(super) struct NoopPersistence;

impl Persistence for NoopPersistence {
    fn save(&self, _data: Vec<(PublicKey, PeerState)>) -> Result<(), NetworkError> {
        Ok(())
    }

    fn load(&self) -> Result<Vec<(PublicKey, PeerState)>, NetworkError> {
        Ok(vec![])
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerData(Vec<(PeerPubKey, PeerState)>);

impl PeerData {
    fn from(peer_box: Vec<(PublicKey, PeerState)>) -> Self {
        let data = peer_box
            .into_iter()
            .map(|(pubkey, state)| (PeerPubKey(pubkey), state))
            .collect::<Vec<_>>();

        PeerData(data)
    }

    fn unbox(self) -> Vec<(PublicKey, PeerState)> {
        self.0
            .into_iter()
            .map(|(pubkey, state)| (pubkey.0, state))
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct PeerPubKey(PublicKey);

impl ser::Serialize for PeerPubKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_bytes(self.0.encode().as_ref())
    }
}

impl<'de> de::Deserialize<'de> for PeerPubKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = PeerPubKey;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("peer pubkey")
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut buf: Vec<u8> = Vec::with_capacity(seq.size_hint().unwrap_or(0));

                while let Some(val) = seq.next_element()? {
                    buf.push(val);
                }

                self.visit_byte_buf(buf)
            }

            fn visit_byte_buf<E: de::Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                self.visit_bytes(v.as_slice())
            }

            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                PublicKey::decode(v)
                    .ok_or_else(|| de::Error::custom("not valid public key"))
                    .map(PeerPubKey)
            }
        }

        deserializer.deserialize_bytes(Visitor)
    }
}
