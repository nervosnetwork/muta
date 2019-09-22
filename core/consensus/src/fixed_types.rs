use std::error::Error;

use bincode::{deserialize, serialize};
use bytes::Bytes;
use overlord::Codec;
use rlp::{Encodable, RlpStream};

use protocol::codec::{Deserialize, ProtocolCodecSync, Serialize};
use protocol::types::{Hash, Pill, Proof, SignedTransaction, Validator};

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

#[derive(Clone, Debug)]
pub struct FixedPill {
    pub inner: Pill,
}

impl From<Pill> for FixedPill {
    fn from(inner: Pill) -> Self {
        FixedPill { inner }
    }
}

impl Encodable for FixedPill {
    fn rlp_append(&self, s: &mut RlpStream) {
        let header = &self.inner.epoch.header;
        let confirm_root = header
            .confirm_root
            .iter()
            .map(|root| root.as_hex())
            .collect::<Vec<_>>();
        let receipt_root = header
            .receipt_root
            .iter()
            .map(|root| root.as_hex())
            .collect::<Vec<_>>();
        let cycles_used = header
            .cycles_used
            .iter()
            .map(|fee| fee.encode_sync().unwrap().as_ref().to_vec())
            .collect::<Vec<_>>();
        let validators = header
            .validators
            .iter()
            .map(|v| FixedValidator::from(v.to_owned()))
            .collect::<Vec<_>>();

        s.begin_list(14)
            .append(&header.chain_id.as_hex())
            .append(&header.epoch_id)
            .append(&header.pre_hash.as_hex())
            .append(&header.timestamp)
            .append(&header.logs_bloom.to_low_u64_be())
            .append(&header.order_root.as_hex())
            .append_list::<String, String>(&confirm_root)
            .append(&header.state_root.as_hex())
            .append_list::<String, String>(&receipt_root)
            .append_list::<Vec<u8>, Vec<u8>>(&cycles_used)
            .append(&header.proposer.as_hex())
            .append(&FixedProof::from(header.proof.clone()))
            .append(&header.validator_version)
            .append_list(&validators);
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

impl Codec for FixedPill {
    fn encode(&self) -> Result<Bytes, Box<dyn Error + Send>> {
        let bytes = self.inner.encode_sync()?;
        Ok(bytes)
    }

    fn decode(data: Bytes) -> Result<Self, Box<dyn Error + Send>> {
        let res = FixedPill::from(Pill::decode_sync(data)?);
        Ok(res)
    }
}

struct FixedValidator {
    inner: Validator,
}

impl From<Validator> for FixedValidator {
    fn from(inner: Validator) -> Self {
        FixedValidator { inner }
    }
}

impl Encodable for FixedValidator {
    fn rlp_append(&self, s: &mut RlpStream) {
        let inner = &self.inner;
        s.begin_list(3)
            .append(&inner.address.as_hex())
            .append(&inner.propose_weight)
            .append(&inner.vote_weight);
    }
}

struct FixedProof {
    inner: Proof,
}

impl From<Proof> for FixedProof {
    fn from(inner: Proof) -> Self {
        FixedProof { inner }
    }
}

impl Encodable for FixedProof {
    fn rlp_append(&self, s: &mut RlpStream) {
        let inner = &self.inner;
        s.begin_list(4)
            .append(&inner.epoch_id)
            .append(&inner.round)
            .append(&inner.epoch_hash.as_hex())
            .append(&inner.signature.as_ref());
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
