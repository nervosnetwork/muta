use super::{ArcPeer, Connectedness, PeerMultiaddr};

use std::{
    convert::TryFrom,
    fmt,
    fs::File,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

use serde::{de, ser};
use serde_derive::{Deserialize, Serialize};
use tentacle::{
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
};

use crate::error::NetworkError;

// TODO: remove skip tag on retry and next_attempt_at
// TODO: save multiaddr failure count
#[derive(Debug, Serialize, Deserialize)]
struct SerdePeer {
    id:              SerdePeerId,
    pubkey:          Option<SerdePubKey>,
    multiaddrs:      Vec<PeerMultiaddr>,
    connectedness:   usize,
    #[serde(skip)]
    retry:           u8,
    #[serde(skip)]
    next_attempt_at: u64,
    connected_at:    u64,
    disconnected_at: u64,
    alive:           u64,
}

impl From<ArcPeer> for SerdePeer {
    fn from(peer: ArcPeer) -> SerdePeer {
        let connectedness = match peer.connectedness() {
            Connectedness::Unconnectable => Connectedness::Unconnectable,
            _ => Connectedness::CanConnect,
        };

        SerdePeer {
            id:              SerdePeerId(peer.owned_id()),
            pubkey:          peer.owned_pubkey().map(SerdePubKey),
            multiaddrs:      peer.multiaddrs.all(),
            connectedness:   connectedness as usize,
            retry:           peer.retry.count(),
            next_attempt_at: peer.retry.next_attempt_at(),
            connected_at:    peer.connected_at(),
            disconnected_at: peer.disconnected_at(),
            alive:           peer.alive(),
        }
    }
}

impl TryFrom<SerdePeer> for ArcPeer {
    type Error = NetworkError;

    fn try_from(serde_peer: SerdePeer) -> Result<Self, Self::Error> {
        let peer_id = serde_peer.id.0;

        let peer = ArcPeer::new(peer_id.clone());
        if let Some(pubkey) = serde_peer.pubkey {
            peer.set_pubkey(pubkey.0)?;
        }

        let multiaddrs = serde_peer
            .multiaddrs
            .into_iter()
            .map(|ma| {
                // Just ensure that our recovered multiaddr has id
                let ma: Multiaddr = ma.into();
                PeerMultiaddr::new(ma, &peer_id)
            })
            .collect();
        peer.multiaddrs.set(multiaddrs);

        peer.set_connectedness(Connectedness::from(serde_peer.connectedness));
        peer.retry.set(serde_peer.retry);
        peer.retry.set_next_attempt_at(serde_peer.next_attempt_at);
        peer.set_connected_at(serde_peer.connected_at);
        peer.set_disconnected_at(serde_peer.disconnected_at);
        peer.set_alive(serde_peer.alive);

        Ok(peer)
    }
}

// TODO: Async support, right now, it's ok since we only restore/save data once.
pub(super) trait SaveRestore: Send + Sync {
    fn save(&self, peers: Vec<ArcPeer>) -> Result<(), NetworkError>;
    fn restore(&self) -> Result<Vec<ArcPeer>, NetworkError>;
}

#[derive(Clone)]
pub(super) struct PeerDatFile {
    path: PathBuf,
}

impl PeerDatFile {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        PeerDatFile {
            path: path.as_ref().to_owned(),
        }
    }
}

impl SaveRestore for PeerDatFile {
    fn save(&self, peers: Vec<ArcPeer>) -> Result<(), NetworkError> {
        let mut file = File::create(&self.path)?;
        let peers_to_save = peers.into_iter().map(SerdePeer::from).collect::<Vec<_>>();
        let data = bincode::serialize(&peers_to_save)?;

        file.write_all(data.as_slice())?;
        Ok(())
    }

    // restore data only happen once during network service starting
    fn restore(&self) -> Result<Vec<ArcPeer>, NetworkError> {
        let file = File::open(&self.path)?;
        let mut buf_reader = BufReader::new(file);
        let mut data = Vec::new();

        buf_reader.read_to_end(&mut data)?;
        let peers_to_restore: Vec<SerdePeer> = bincode::deserialize(&data)?;

        let mut peers = Vec::with_capacity(peers_to_restore.len());
        for p in peers_to_restore {
            if let Ok(p) = ArcPeer::try_from(p) {
                peers.push(p);
            }
        }

        Ok(peers)
    }
}

#[derive(Clone)]
pub(super) struct NoPeerDatFile;

impl SaveRestore for NoPeerDatFile {
    fn save(&self, _peers: Vec<ArcPeer>) -> Result<(), NetworkError> {
        Ok(())
    }

    fn restore(&self) -> Result<Vec<ArcPeer>, NetworkError> {
        Ok(vec![])
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct SerdePubKey(PublicKey);

impl ser::Serialize for SerdePubKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_bytes(self.0.clone().encode().as_ref())
    }
}

impl<'de> de::Deserialize<'de> for SerdePubKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SerdePubKey;

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
                    .map(SerdePubKey)
            }
        }

        deserializer.deserialize_bytes(Visitor)
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct SerdePeerId(PeerId);

impl ser::Serialize for SerdePeerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_bytes(self.0.as_bytes())
    }
}

impl<'de> de::Deserialize<'de> for SerdePeerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SerdePeerId;

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
                PeerId::from_bytes(v.to_vec())
                    .map_err(|_| de::Error::custom("not valid peer id"))
                    .map(SerdePeerId)
            }
        }

        deserializer.deserialize_bytes(Visitor)
    }
}
