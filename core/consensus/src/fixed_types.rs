use std::error::Error;

use async_trait::async_trait;
use bincode::{deserialize, serialize};
use bytes::Bytes;
use overlord::Codec;

use protocol::codec::{Deserialize, ProtocolCodecSync, Serialize};
use protocol::fixed_codec::ProtocolFixedCodec;
use protocol::types::{Epoch, Hash, Pill, SignedTransaction};
use protocol::{traits::MessageCodec, ProtocolResult};

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
    use num_traits::FromPrimitive;
    use rand::random;

    use protocol::types::{
        CarryingAsset, Epoch, EpochHeader, Fee, Hash, Proof, RawTransaction, SignedTransaction,
        TransactionAction, UserAddress,
    };

    use super::{Bytes, FixedEpoch, FixedSignedTxs};

    fn gen_epoch(epoch_id: u64, epoch_hash: Hash) -> Epoch {
        let nonce = Hash::digest(Bytes::from("XXXX"));
        let addr_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let header = EpochHeader {
            chain_id: nonce.clone(),
            epoch_id,
            pre_hash: nonce.clone(),
            timestamp: 1000,
            logs_bloom: Default::default(),
            order_root: nonce.clone(),
            confirm_root: Vec::new(),
            state_root: nonce,
            receipt_root: Vec::new(),
            cycles_used: vec![999_999],
            proposer: UserAddress::from_hex(addr_str).unwrap(),
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

    fn gen_user_address() -> UserAddress {
        let inner = "0x107899EE7319601cbC2684709e0eC3A4807bb0Fd74";
        UserAddress::from_hex(inner).unwrap()
    }

    fn gen_signed_tx() -> SignedTransaction {
        use protocol::codec::ProtocolCodec;

        let address = gen_user_address();
        let nonce = Hash::digest(Bytes::from(gen_random_bytes(10)));
        let fee = Fee {
            asset_id: nonce.clone(),
            cycle:    random::<u64>(),
        };
        let action = TransactionAction::Transfer {
            receiver:       address,
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

    #[runtime::test]
    async fn test_epoch_codec() {
        use super::MessageCodec;

        let epoch = gen_epoch(random::<u64>(), Hash::from_empty());
        let mut origin = FixedEpoch::new(epoch.clone());
        let bytes = origin.encode().await.unwrap();
        let res: FixedEpoch = MessageCodec::decode(bytes).await.unwrap();
        assert_eq!(res.inner, epoch);
    }
}
