use std::convert::TryFrom;

use bytes::Bytes;
use prost::Message;

use crate::{
    codec::{
        primitive::{Address, Hash},
        CodecError, ProtocolCodecSync,
    },
    field, impl_default_bytes_codec_for,
    types::primitive as protocol_primitive,
    ProtocolError, ProtocolResult,
};

// #####################
// Protobuf
// #####################

#[derive(Clone, Message)]
pub struct Block {
    #[prost(message, tag = "1")]
    pub header: Option<BlockHeader>,

    #[prost(message, repeated, tag = "2")]
    pub ordered_tx_hashes: Vec<Hash>,
}

#[derive(Clone, Message)]
pub struct BlockHeader {
    #[prost(message, tag = "1")]
    pub chain_id: Option<Hash>,

    #[prost(uint64, tag = "2")]
    pub height: u64,

    #[prost(message, tag = "3")]
    pub prev_hash: Option<Hash>,

    #[prost(uint64, tag = "4")]
    pub timestamp: u64,

    #[prost(message, tag = "5")]
    pub order_root: Option<Hash>,

    #[prost(message, tag = "6")]
    pub order_signed_transactions_hash: Option<Hash>,

    #[prost(message, repeated, tag = "7")]
    pub confirm_root: Vec<Hash>,

    #[prost(message, tag = "8")]
    pub state_root: Option<Hash>,

    #[prost(message, repeated, tag = "9")]
    pub receipt_root: Vec<Hash>,

    #[prost(message, repeated, tag = "10")]
    pub cycles_used: Vec<u64>,

    #[prost(message, tag = "11")]
    pub proposer: Option<Address>,

    #[prost(message, tag = "12")]
    pub proof: Option<Proof>,

    #[prost(uint64, tag = "13")]
    pub validator_version: u64,

    #[prost(message, repeated, tag = "14")]
    pub validators: Vec<Validator>,

    #[prost(uint64, tag = "15")]
    pub exec_height: u64,
}

#[derive(Clone, Message)]
pub struct Proof {
    #[prost(uint64, tag = "1")]
    pub height: u64,

    #[prost(uint64, tag = "2")]
    pub round: u64,

    #[prost(message, tag = "3")]
    pub block_hash: Option<Hash>,

    #[prost(bytes, tag = "4")]
    pub signature: Vec<u8>,

    #[prost(bytes, tag = "5")]
    pub bitmap: Vec<u8>,
}

#[derive(Clone, Message)]
pub struct Validator {
    #[prost(bytes, tag = "1")]
    pub peer_id: Vec<u8>,

    #[prost(uint32, tag = "2")]
    pub propose_weight: u32,

    #[prost(uint32, tag = "3")]
    pub vote_weight: u32,
}

#[derive(Clone, Message)]
pub struct Pill {
    #[prost(message, tag = "1")]
    pub block: Option<Block>,

    #[prost(message, repeated, tag = "2")]
    pub propose_hashes: Vec<Hash>,
}

// #################
// Conversion
// #################

// Block

impl From<block::Block> for Block {
    fn from(block: block::Block) -> Block {
        let header = Some(BlockHeader::from(block.header));
        let ordered_tx_hashes = block
            .ordered_tx_hashes
            .into_iter()
            .map(Hash::from)
            .collect::<Vec<_>>();

        Block {
            header,
            ordered_tx_hashes,
        }
    }
}

impl TryFrom<Block> for block::Block {
    type Error = ProtocolError;

    fn try_from(block: Block) -> Result<block::Block, Self::Error> {
        let header = field!(block.header, "Block", "header")?;

        let mut ordered_tx_hashes = Vec::new();
        for hash in block.ordered_tx_hashes {
            ordered_tx_hashes.push(protocol_primitive::Hash::try_from(hash)?);
        }

        let block = block::Block {
            header: block::BlockHeader::try_from(header)?,
            ordered_tx_hashes,
        };

        Ok(block)
    }
}

// BlockHeader

impl From<block::BlockHeader> for BlockHeader {
    fn from(block_header: block::BlockHeader) -> BlockHeader {
        let chain_id = Some(Hash::from(block_header.chain_id));
        let prev_hash = Some(Hash::from(block_header.prev_hash));
        let order_root = Some(Hash::from(block_header.order_root));
        let order_signed_transactions_hash =
            Some(Hash::from(block_header.order_signed_transactions_hash));
        let state_root = Some(Hash::from(block_header.state_root));
        let proposer = Some(Address::from(block_header.proposer));
        let proof = Some(Proof::from(block_header.proof));

        let confirm_root = block_header
            .confirm_root
            .into_iter()
            .map(Hash::from)
            .collect::<Vec<_>>();
        let receipt_root = block_header
            .receipt_root
            .into_iter()
            .map(Hash::from)
            .collect::<Vec<_>>();
        let validators = block_header
            .validators
            .into_iter()
            .map(Validator::from)
            .collect::<Vec<_>>();

        BlockHeader {
            chain_id,
            height: block_header.height,
            exec_height: block_header.exec_height,
            prev_hash,
            timestamp: block_header.timestamp,
            order_root,
            order_signed_transactions_hash,
            confirm_root,
            state_root,
            receipt_root,
            cycles_used: block_header.cycles_used,
            proposer,
            proof,
            validator_version: block_header.validator_version,
            validators,
        }
    }
}

impl TryFrom<BlockHeader> for block::BlockHeader {
    type Error = ProtocolError;

    fn try_from(block_header: BlockHeader) -> Result<block::BlockHeader, Self::Error> {
        let chain_id = field!(block_header.chain_id, "BlockHeader", "chain_id")?;
        let prev_hash = field!(block_header.prev_hash, "BlockHeader", "prev_hash")?;
        let order_root = field!(block_header.order_root, "BlockHeader", "order_root")?;
        let order_signed_transactions_hash = field!(
            block_header.order_signed_transactions_hash,
            "BlockHeader",
            "order_signed_transactions_hash"
        )?;
        let state_root = field!(block_header.state_root, "BlockHeader", "state_root")?;
        let proposer = field!(block_header.proposer, "BlockHeader", "proposer")?;
        let proof = field!(block_header.proof, "BlockHeader", "proof")?;

        let mut confirm_root = Vec::new();
        for root in block_header.confirm_root {
            confirm_root.push(protocol_primitive::Hash::try_from(root)?);
        }

        let mut receipt_root = Vec::new();
        for root in block_header.receipt_root {
            receipt_root.push(protocol_primitive::Hash::try_from(root)?);
        }

        let mut validators = Vec::new();
        for validator in block_header.validators {
            validators.push(block::Validator::try_from(validator)?);
        }

        let proof = block::BlockHeader {
            chain_id: protocol_primitive::Hash::try_from(chain_id)?,
            height: block_header.height,
            exec_height: block_header.exec_height,
            prev_hash: protocol_primitive::Hash::try_from(prev_hash)?,
            timestamp: block_header.timestamp,
            order_root: protocol_primitive::Hash::try_from(order_root)?,
            order_signed_transactions_hash: protocol_primitive::Hash::try_from(
                order_signed_transactions_hash,
            )?,
            confirm_root,
            state_root: protocol_primitive::Hash::try_from(state_root)?,
            receipt_root,
            cycles_used: block_header.cycles_used,
            proposer: protocol_primitive::Address::try_from(proposer)?,
            proof: block::Proof::try_from(proof)?,
            validator_version: block_header.validator_version,
            validators,
        };

        Ok(proof)
    }
}

// Proof

impl From<block::Proof> for Proof {
    fn from(proof: block::Proof) -> Proof {
        let block_hash = Some(Hash::from(proof.block_hash));

        Proof {
            height: proof.height,
            round: proof.round,
            block_hash,
            signature: proof.signature.to_vec(),
            bitmap: proof.bitmap.to_vec(),
        }
    }
}

impl TryFrom<Proof> for block::Proof {
    type Error = ProtocolError;

    fn try_from(proof: Proof) -> Result<block::Proof, Self::Error> {
        let block_hash = field!(proof.block_hash, "Proof", "block_hash")?;

        let proof = block::Proof {
            height:     proof.height,
            round:      proof.round,
            block_hash: protocol_primitive::Hash::try_from(block_hash)?,
            signature:  Bytes::from(proof.signature),
            bitmap:     Bytes::from(proof.bitmap),
        };

        Ok(proof)
    }
}

// Validator

impl From<block::Validator> for Validator {
    fn from(validator: block::Validator) -> Validator {
        Validator {
            peer_id:        validator.peer_id.to_vec(),
            propose_weight: validator.propose_weight,
            vote_weight:    validator.vote_weight,
        }
    }
}

impl TryFrom<Validator> for block::Validator {
    type Error = ProtocolError;

    fn try_from(validator: Validator) -> Result<block::Validator, Self::Error> {
        let validator = block::Validator {
            peer_id:        Bytes::from(validator.peer_id),
            propose_weight: validator.propose_weight,
            vote_weight:    validator.vote_weight,
        };

        Ok(validator)
    }
}

// Pill

impl From<block::Pill> for Pill {
    fn from(pill: block::Pill) -> Pill {
        let block = Some(Block::from(pill.block));
        let propose_hashes = pill
            .propose_hashes
            .into_iter()
            .map(Hash::from)
            .collect::<Vec<_>>();

        Pill {
            block,
            propose_hashes,
        }
    }
}

impl TryFrom<Pill> for block::Pill {
    type Error = ProtocolError;

    fn try_from(pill: Pill) -> Result<block::Pill, Self::Error> {
        let block = field!(pill.block, "Pill", "block")?;

        let mut propose_hashes = Vec::new();
        for hash in pill.propose_hashes {
            propose_hashes.push(protocol_primitive::Hash::try_from(hash)?);
        }

        let pill = block::Pill {
            block: block::Block::try_from(block)?,
            propose_hashes,
        };

        Ok(pill)
    }
}

// #################
// Codec
// #################

impl_default_bytes_codec_for!(block, [Block, BlockHeader, Proof, Validator, Pill]);

#[cfg(test)]
mod test {
    #[test]
    fn test_u8_convert_u32() {
        for i in u8::min_value()..u8::max_value() {
            let j = u32::from(i);
            assert_eq!(i, (j as u8));
        }
    }
}
