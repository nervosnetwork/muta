use bytes::BytesMut;

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::block::{Block, BlockHeader, Pill, Proof, Validator};
use crate::types::primitive::Hash;
use crate::types::Bloom;
use crate::{impl_default_fixed_codec_for, ProtocolResult};

// Impl FixedCodec trait for types
impl_default_fixed_codec_for!(block, [Proof, Validator, Block, BlockHeader, Pill]);

impl rlp::Encodable for Proof {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(5)
            .append(&self.bitmap.to_vec())
            .append(&self.block_hash)
            .append(&self.height)
            .append(&self.round)
            .append(&self.signature.to_vec());
    }
}

impl rlp::Decodable for Proof {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 5 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let bitmap = BytesMut::from(r.at(0)?.data()?).freeze();
        let block_hash: Hash = rlp::decode(r.at(1)?.as_raw())?;
        let height = r.at(2)?.as_val()?;
        let round = r.at(3)?.as_val()?;
        let signature = BytesMut::from(r.at(4)?.data()?).freeze();

        Ok(Proof {
            height,
            round,
            block_hash,
            signature,
            bitmap,
        })
    }
}

impl rlp::Encodable for Validator {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3)
            .append(&self.address)
            .append(&self.propose_weight)
            .append(&self.vote_weight);
    }
}

impl rlp::Decodable for Validator {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 3 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let address = rlp::decode(r.at(0)?.as_raw())?;
        let propose_weight = r.at(1)?.as_val()?;
        let vote_weight = r.at(2)?.as_val()?;

        Ok(Validator {
            address,
            propose_weight,
            vote_weight,
        })
    }
}

impl rlp::Encodable for BlockHeader {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(15)
            .append(&self.chain_id)
            .append_list(&self.confirm_root)
            .append_list(&self.cycles_used)
            .append(&self.height)
            .append(&self.exec_height)
            .append_list(&self.logs_bloom)
            .append(&self.order_root)
            .append(&self.pre_hash)
            .append(&self.proof)
            .append(&self.proposer)
            .append_list(&self.receipt_root)
            .append(&self.state_root)
            .append(&self.timestamp)
            .append(&self.validator_version)
            .append_list(&self.validators);
    }
}

impl rlp::Decodable for BlockHeader {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 15 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let chain_id: Hash = rlp::decode(r.at(0)?.as_raw())?;
        let confirm_root: Vec<Hash> = rlp::decode_list(r.at(1)?.as_raw());
        let cycles_used: Vec<u64> = rlp::decode_list(r.at(2)?.as_raw());
        let height: u64 = r.at(3)?.as_val()?;
        let exec_height: u64 = r.at(4)?.as_val()?;
        let logs_bloom: Vec<Bloom> = rlp::decode_list(r.at(5)?.as_raw());
        let order_root = rlp::decode(r.at(6)?.as_raw())?;
        let pre_hash = rlp::decode(r.at(7)?.as_raw())?;
        let proof: Proof = rlp::decode(r.at(8)?.as_raw())?;
        let proposer = rlp::decode(r.at(9)?.as_raw())?;
        let receipt_root: Vec<Hash> = rlp::decode_list(r.at(10)?.as_raw());
        let state_root = rlp::decode(r.at(11)?.as_raw())?;
        let timestamp: u64 = r.at(12)?.as_val()?;
        let validator_version: u64 = r.at(13)?.as_val()?;
        let validators: Vec<Validator> = rlp::decode_list(r.at(14)?.as_raw());

        Ok(BlockHeader {
            chain_id,
            height,
            exec_height,
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
            validators,
        })
    }
}

impl rlp::Encodable for Block {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2)
            .append(&self.header)
            .append_list(&self.ordered_tx_hashes);
    }
}

impl rlp::Decodable for Block {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let header: BlockHeader = rlp::decode(r.at(0)?.as_raw())?;
        let ordered_tx_hashes: Vec<Hash> = rlp::decode_list(r.at(1)?.as_raw());

        Ok(Block {
            header,
            ordered_tx_hashes,
        })
    }
}

impl rlp::Encodable for Pill {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2)
            .append(&self.block)
            .append_list(&self.propose_hashes);
    }
}

impl rlp::Decodable for Pill {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let block: Block = rlp::decode(r.at(0)?.as_raw())?;
        let propose_hashes: Vec<Hash> = rlp::decode_list(r.at(1)?.as_raw());

        Ok(Pill {
            block,
            propose_hashes,
        })
    }
}
