use bytes::Bytes;

use crate::{ProtocolResult, impl_default_fixed_codec_for};
use crate::fixed_codec::{FixedCodecError, ProtocolFixedCodec};
use crate::types::{
    Bloom,
    primitive::{Hash, UserAddress},
    epoch::{Proof, Validator, Epoch, EpochHeader, Pill},
};

impl_default_fixed_codec_for!(epoch, [Proof, Validator, Epoch, EpochHeader, Pill]);

impl rlp::Encodable for Proof {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(5)
            .append(&self.bitmap.to_vec())
            .append(&self.epoch_hash)
            .append(&self.epoch_id)
            .append(&self.round)
            .append(&self.signature.to_vec());
    }
}

impl rlp::Decodable for Proof {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 5 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let mut values = Vec::with_capacity(5);

        for val in r {
            let data = val.data()?;
            values.push(data)
        }

        let bitmap = Bytes::from(values[0]);
        // let epoch_hash = Hash::from_bytes(Bytes::from(values[1]))
        //    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let epoch_hash: Hash = rlp::decode(r.at(1)?.as_raw())?;
        let epoch_id = r.at(2)?.as_val()?;
        let round = r.at(3)?.as_val()?;
        let signature = Bytes::from(values[4]);

        Ok(Proof {
            epoch_id,
            round,
            epoch_hash,
            signature,
            bitmap
        })
    }
}

impl rlp::Encodable for Validator {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3)
            .append(&self.address.as_bytes().to_vec())
            .append(&self.propose_weight)
            .append(&self.vote_weight);
    }
}

impl rlp::Decodable for Validator {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 3 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let address = UserAddress::from_bytes(Bytes::from(r.at(0)?.data()?))
            .map_err(|_|rlp::DecoderError::RlpInvalidLength)?;
        let propose_weight = r.at(1)?.as_val()?;
        let vote_weight = r.at(2)?.as_val()?;

        Ok(Validator {
            address,
            propose_weight,
            vote_weight
        })
    }
}

impl rlp::Encodable for EpochHeader {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        // let confirm_root = self
        //     .confirm_root
        //     .iter()
        //     .map(|root| root.as_bytes().to_vec())
        //     .collect::<Vec<_>>();
        // let receipt_root = self
        //     .receipt_root
        //     .iter()
        //     .map(|root| root.as_bytes().to_vec())
        //     .collect::<Vec<_>>();

        s.begin_list(14)
            .append(&self.chain_id)
            .append_list(&self.confirm_root)
            .append(&self.cycles_used)
            .append(&self.epoch_id)
            .append(&self.logs_bloom)
            .append(&self.order_root)
            .append(&self.pre_hash)
            .append(&self.proof)
            .append(&self.proposer.as_bytes().to_vec())
            .append_list(&self.receipt_root)
            .append(&self.state_root)
            .append(&self.timestamp)
            .append(&self.validator_version)
            .append_list(&self.validators);
    }
}

impl rlp::Decodable for EpochHeader {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 14 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let mut values = Vec::with_capacity(14);

        for val in r {
            let data = val.data()?;
            values.push(data)
        }

        let chain_id: Hash = rlp::decode(r.at(0)?.as_raw())?;
        let confirm_root: Vec<Hash> = rlp::decode_list(r.at(1)?.as_raw());
        let cycles_used: u64 = r.at(2)?.as_val()?;
        let epoch_id: u64 = r.at(3)?.as_val()?;
        // let logs_bloom = Bloom::from_slice(values[4]);
        let logs_bloom: Bloom = rlp::decode(r.at(4)?.as_raw())?;
        let order_root = rlp::decode(r.at(5)?.as_raw())?;
        let pre_hash = rlp::decode(r.at(6)?.as_raw())?;
        let proof: Proof = rlp::decode(r.at(7)?.as_raw())?;
        let proposer = UserAddress::from_bytes(Bytes::from(values[8]))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let receipt_root: Vec<Hash> = rlp::decode_list(r.at(9)?.as_raw());
        let state_root = rlp::decode(r.at(10)?.as_raw())?;
        let timestamp: u64 = r.at(11)?.as_val()?;
        let validator_version: u64 = r.at(12)?.as_val()?;
        let validators: Vec<Validator> = rlp::decode_list(r.at(13)?.as_raw());

        //TODO: fix unwarp
        // let confirm_root = confirm_root
        //     .iter()
        //     .map(|root| MerkleRoot::from_hex(&root).unwrap())
        //     .collect::<Vec<_>>();
        // let receipt_root = receipt_root
        //     .iter()
        //     .map(|root| MerkleRoot::from_hex(&root).unwrap())
        //     .collect::<Vec<_>>();

        Ok(EpochHeader {
            chain_id,
            epoch_id,
            pre_hash,
            timestamp,
            logs_bloom,
            order_root,
            confirm_root,
            state_root,
            receipt_root,
            cycles_used,
            proposer,
            proof,
            validator_version,
            validators
        })
    }
}

impl rlp::Encodable for Epoch {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        // let ordered_tx_hashes = self
        //     .ordered_tx_hashes
        //     .iter()
        //     .map(|root| root.as_hex())
        //     .collect::<Vec<_>>();

        s.begin_list(2)
            .append(&self.header)
            .append_list(&self.ordered_tx_hashes);
    }
}

impl rlp::Decodable for Epoch {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let header: EpochHeader = rlp::decode(r.at(0)?.as_raw())?;
        let ordered_tx_hashes: Vec<Hash> = rlp::decode_list(r.at(1)?.as_raw());

        //TODO: fix unwrap
        // let ordered_tx_hashes = ordered_tx_hashes
        //     .iter()
        //     .map(|hash| Hash::from_hex(&hash).unwrap())
        //     .collect::<Vec<_>>();

        Ok(Epoch {
            header,
            ordered_tx_hashes
        })    
    }
}

impl rlp::Encodable for Pill {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2)
            .append(&self.epoch)
            .append_list(&self.propose_hashes);
    }
}

impl rlp::Decodable for Pill {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let epoch: Epoch = rlp::decode(r.at(0)?.as_raw())?;
        let propose_hashes: Vec<Hash> = rlp::decode_list(r.at(1)?.as_raw());

        Ok(Pill {
            epoch,
            propose_hashes
        })  
    }
}