use super::{ArcPeer, Connectedness};

use std::{
    convert::TryFrom,
    fmt,
    fs::File,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

use serde::{de, ser};
use serde_derive::{Deserialize, Serialize};
use tentacle::{multiaddr::Multiaddr, secio::PublicKey};

use crate::{error::NetworkError, traits::MultiaddrExt};

// TODO: remove skip tag on retry and next_attempt
#[derive(Debug, Serialize, Deserialize)]
struct SerdePeer {
    pubkey:          PeerPubKey,
    multiaddrs:      Vec<Multiaddr>,
    connectedness:   usize,
    #[serde(skip)]
    retry:           u8,
    #[serde(skip)]
    next_attempt:    u64,
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
            pubkey:          PeerPubKey(peer.pubkey.as_ref().to_owned()),
            multiaddrs:      peer.multiaddrs(),
            connectedness:   connectedness as usize,
            retry:           peer.retry(),
            next_attempt:    peer.next_attempt(),
            connected_at:    peer.connected_at(),
            disconnected_at: peer.disconnected_at(),
            alive:           peer.alive(),
        }
    }
}

impl TryFrom<SerdePeer> for ArcPeer {
    type Error = NetworkError;

    fn try_from(serde_peer: SerdePeer) -> Result<Self, Self::Error> {
        let pid = serde_peer.pubkey.0.peer_id();
        let peer = ArcPeer::from_pubkey(serde_peer.pubkey.0)?;

        let multiaddrs = serde_peer
            .multiaddrs
            .into_iter()
            .map(|mut ma| {
                if !ma.has_id() {
                    ma.push_id(pid.clone())
                }
                ma
            })
            .collect();
        peer.set_multiaddrs(multiaddrs);

        peer.set_connectedness(Connectedness::from(serde_peer.connectedness));
        peer.set_retry(serde_peer.retry);
        peer.set_next_attempt(serde_peer.next_attempt);
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
