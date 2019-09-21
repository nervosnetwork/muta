use std::error::Error;

use bincode::{deserialize, serialize};
use bytes::Bytes;
use overlord::Codec;
use rlp::{Encodable, RlpStream};

use protocol::codec::{Deserialize, ProtocolCodecSync, Serialize};
use protocol::types::{Hash, Pill, Proof, SignedTransaction, Validator};

#[derive(Serialize, Deserialize, Clone, Debug)]
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
            .map(|mkr| mkr.as_hex())
            .collect::<Vec<_>>();
        let receipt_root = header
            .receipt_root
            .iter()
            .map(|mkr| mkr.as_hex())
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
            .append(&header.cycles_used)
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
