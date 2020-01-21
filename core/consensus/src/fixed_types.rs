use std::error::Error;

use async_trait::async_trait;
use bincode::{deserialize, serialize};
use overlord::Codec;

use protocol::codec::{Deserialize, ProtocolCodecSync, Serialize};
use protocol::fixed_codec::FixedCodec;
use protocol::types::{Epoch, Hash, Pill, SignedTransaction};
use protocol::{traits::MessageCodec, Bytes, BytesMut, ProtocolResult};

use crate::{ConsensusError, MsgType};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ConsensusRpcRequest {
    PullEpochs(u64),
    PullTxs(PullTxsRequest),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusRpcResponse {
    PullEpochs(Box<Epoch>),
    PullTxs(Box<FixedSignedTxs>),
}

#[async_trait]
impl MessageCodec for ConsensusRpcResponse {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        let bytes = match self {
            ConsensusRpcResponse::PullEpochs(ep) => {
                let mut tmp = BytesMut::from(ep.encode_fixed()?.as_ref());
                tmp.extend_from_slice(b"a");
                tmp
            }

            ConsensusRpcResponse::PullTxs(txs) => {
                let mut tmp = BytesMut::from(
                    serialize(&txs)
                        .map_err(|_| ConsensusError::EncodeErr(MsgType::RpcPullTxs))?
                        .as_slice(),
                );
                tmp.extend_from_slice(b"b");
                tmp
            }
        };
        Ok(bytes.freeze())
    }

    async fn decode(mut bytes: Bytes) -> ProtocolResult<Self> {
        let len = bytes.len();
        let flag = bytes.split_off(len - 1);

        match flag.as_ref() {
            b"a" => {
                let res: Epoch = FixedCodec::decode_fixed(bytes)?;
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
        let inner: Pill = FixedCodec::decode_fixed(data)?;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedEpoch {
    pub inner: Epoch,
}

#[async_trait]
impl MessageCodec for FixedEpoch {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        self.inner.encode_sync()
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        let inner: Epoch = ProtocolCodecSync::decode_sync(bytes)?;
        Ok(FixedEpoch::new(inner))
    }
}

impl FixedEpoch {
    pub fn new(inner: Epoch) -> Self {
        FixedEpoch { inner }
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

    use futures::executor;
    use rand::random;

    use protocol::types::{
        Address, Epoch, EpochHeader, Hash, Proof, RawTransaction, SignedTransaction,
        TransactionRequest,
    };
    use protocol::Bytes;

    use super::{FixedEpoch, FixedSignedTxs};

    fn gen_epoch(epoch_id: u64, epoch_hash: Hash) -> Epoch {
        let nonce = Hash::digest(Bytes::from("XXXX"));
        let addr_str = "CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let header = EpochHeader {
            chain_id: nonce.clone(),
            epoch_id,
            exec_epoch_id: epoch_id - 1,
            pre_hash: nonce.clone(),
            timestamp: 1000,
            logs_bloom: Default::default(),
            order_root: nonce.clone(),
            confirm_root: Vec::new(),
            state_root: nonce,
            receipt_root: Vec::new(),
            cycles_used: vec![999_999],
            proposer: Address::from_hex(addr_str).unwrap(),
            proof: mock_proof(epoch_hash),
            validator_version: 1,
            validators: Vec::new(),
        };

        Epoch {
            header,
            ordered_tx_hashes: Vec::new(),
        }
    }

    fn mock_proof(epoch_hash: Hash) -> Proof {
        Proof {
            epoch_id: 0,
            round: 0,
            epoch_hash,
            signature: Default::default(),
            bitmap: Default::default(),
        }
    }

    fn gen_random_bytes(len: usize) -> Vec<u8> {
        (0..len).map(|_| random::<u8>()).collect::<Vec<_>>()
    }

    fn gen_signed_tx() -> SignedTransaction {
        use protocol::codec::ProtocolCodec;

        let nonce = Hash::digest(Bytes::from(gen_random_bytes(10)));

        let request = TransactionRequest {
            service_name: "test".to_owned(),
            method:       "test".to_owned(),
            payload:      "test".to_owned(),
        };
        let mut raw = RawTransaction {
            chain_id: nonce.clone(),
            nonce,
            timeout: random::<u64>(),
            cycles_price: 1,
            cycles_limit: random::<u64>(),
            request,
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
    fn test_txs_codec() {
        use super::Codec;
        for _ in 0..10 {
            let fixed_txs = FixedSignedTxs {
                inner: (0..1000).map(|_| gen_signed_tx()).collect::<Vec<_>>(),
            };

            let bytes = fixed_txs.encode().unwrap();
            assert_eq!(fixed_txs, FixedSignedTxs::decode(bytes).unwrap());
        }
    }

    #[tokio::test]
    async fn test_epoch_codec() {
        use super::MessageCodec;

        let epoch = gen_epoch(random::<u64>(), Hash::from_empty());
        let mut origin = FixedEpoch::new(epoch.clone());
        let bytes = origin.encode().await.unwrap();
        let res: FixedEpoch = MessageCodec::decode(bytes).await.unwrap();
        assert_eq!(res.inner, epoch);
    }
}
