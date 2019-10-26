use std::error::Error;

use async_trait::async_trait;
use bincode::{deserialize, serialize};
use bytes::Bytes;
use overlord::Codec;

use protocol::codec::{Deserialize, Serialize};
use protocol::traits::MessageCodec;
use protocol::types::{Epoch, Hash, Pill, SignedTransaction};
use protocol::{fixed_codec::ProtocolFixedCodec, ProtocolResult};

use crate::{ConsensusError, MsgType};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ConsensusRpcRequest {
    PullEpochs(u64),
    PullTxs(PullTxsRequest),
}

#[derive(Clone, Debug)]
pub enum ConsensusRpcResponse {
    PullEpochs(Box<Epoch>),
    PullTxs(Box<FixedSignedTxs>),
}

#[async_trait]
impl MessageCodec for ConsensusRpcResponse {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        let bytes = match self {
            ConsensusRpcResponse::PullEpochs(ep) => {
                let mut tmp = ep.encode_fixed()?;
                tmp.extend_from_slice(b"a");
                tmp
            }

            ConsensusRpcResponse::PullTxs(txs) => {
                let mut tmp = Bytes::from(
                    serialize(&txs).map_err(|_| ConsensusError::EncodeErr(MsgType::RpcPullTxs))?,
                );
                tmp.extend_from_slice(b"b");
                tmp
            }
        };
        Ok(bytes)
    }

    async fn decode(mut bytes: Bytes) -> ProtocolResult<Self> {
        let flag = bytes.split_to(1);
        match flag.as_ref() {
            b"a" => {
                let res: Epoch = ProtocolFixedCodec::decode_fixed(bytes)?;
                Ok(ConsensusRpcResponse::PullEpochs(Box::new(res)))
            }

            b"b" => {
                let res: FixedSignedTxs = deserialize(&bytes)
                    .map_err(|_| ConsensusError::DecodeErr(MsgType::RpcPullTxs))?;
                Ok(ConsensusRpcResponse::PullTxs(Box::new(res)))
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedPill {
    pub inner: Pill,
}

impl Codec for FixedPill {
    fn encode(&self) -> Result<Bytes, Box<dyn Error + Send>> {
        let bytes = self.inner.encode_fixed()?;
        Ok(bytes)
    }

    fn decode(data: Bytes) -> Result<Self, Box<dyn Error + Send>> {
        let inner: Pill = ProtocolFixedCodec::decode_fixed(data)?;
        Ok(FixedPill { inner })
    }
}

impl FixedPill {
    pub fn get_ordered_hashes(&self) -> Vec<Hash> {
        self.inner.epoch.ordered_tx_hashes.clone()
    }

    pub fn get_propose_hashes(&self) -> Vec<Hash> {
        self.inner.propose_hashes.clone()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FixedEpochID {
    pub inner: u64,
}

impl FixedEpochID {
    pub fn new(inner: u64) -> Self {
        FixedEpochID { inner }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PullTxsRequest {
    #[serde(with = "core_network::serde_multi")]
    pub inner: Vec<Hash>,
}

impl PullTxsRequest {
    pub fn new(inner: Vec<Hash>) -> Self {
        PullTxsRequest { inner }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FixedSignedTxs {
    #[serde(with = "core_network::serde_multi")]
    pub inner: Vec<SignedTransaction>,
}

impl Codec for FixedSignedTxs {
    fn encode(&self) -> Result<Bytes, Box<dyn Error + Send>> {
        let bytes = serialize(&self).map_err(|e| e as Box<dyn Error + Send>)?;
        Ok(Bytes::from(bytes))
    }

    fn decode(data: Bytes) -> Result<Self, Box<dyn Error + Send>> {
        let res: FixedSignedTxs =
            deserialize(data.as_ref()).map_err(|e| e as Box<dyn Error + Send>)?;
        Ok(res)
    }
}

impl FixedSignedTxs {
    pub fn new(inner: Vec<SignedTransaction>) -> Self {
        FixedSignedTxs { inner }
    }
}

#[cfg(test)]
mod test {
    use std::convert::From;

    use bytes::Bytes;
    use futures::executor;
    use num_traits::FromPrimitive;
    use overlord::Codec;
    use rand::random;

    use protocol::codec::ProtocolCodec;
    use protocol::types::{
        CarryingAsset, Fee, Hash, RawTransaction, SignedTransaction, TransactionAction, UserAddress,
    };

    use super::FixedSignedTxs;

    fn gen_random_bytes(len: usize) -> Vec<u8> {
        (0..len).map(|_| random::<u8>()).collect::<Vec<_>>()
    }

    fn gen_user_address() -> UserAddress {
        let inner = "0x107899EE7319601cbC2684709e0eC3A4807bb0Fd74";
        UserAddress::from_hex(inner).unwrap()
    }

    fn gen_signed_tx() -> SignedTransaction {
        let address = gen_user_address();
        let nonce = Hash::digest(Bytes::from(gen_random_bytes(10)));
        let fee = Fee {
            asset_id: nonce.clone(),
            cycle:    random::<u64>(),
        };
        let action = TransactionAction::Transfer {
            receiver:       address.clone(),
            carrying_asset: CarryingAsset {
                asset_id: nonce.clone(),
                amount:   FromPrimitive::from_i32(42).unwrap(),
            },
        };
        let mut raw = RawTransaction {
            chain_id: nonce.clone(),
            nonce,
            timeout: random::<u64>(),
            fee,
            action,
        };

        let raw_bytes = executor::block_on(async { raw.encode().await.unwrap() });
        let tx_hash = Hash::digest(raw_bytes);

        SignedTransaction {
            raw,
            tx_hash,
            pubkey: Bytes::from(gen_random_bytes(32)),
            signature: Bytes::from(gen_random_bytes(64)),
        }
    }

    #[test]
    fn test_codec() {
        for _ in 0..10 {
            let fixed_txs = FixedSignedTxs {
                inner: (0..1000).map(|_| gen_signed_tx()).collect::<Vec<_>>(),
            };

            let bytes = fixed_txs.encode().unwrap();
            assert_eq!(fixed_txs, FixedSignedTxs::decode(bytes).unwrap());
        }
    }
}
