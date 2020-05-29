use protocol::fixed_codec::FixedCodec;
use protocol::types::Hash as PHash;

use crate::schema::{Address, Bytes, Hash, MerkleRoot, Uint64};

#[derive(juniper::GraphQLObject, Clone)]
#[graphql(
    description = "Block is a single digital record created within a blockchain. \
                   Each block contains a record of the previous Block, \
                   and when linked together these become the “chain”.\
                   A block is always composed of header and body."
)]
pub struct Block {
    #[graphql(description = "The header section of a block")]
    header:            BlockHeader,
    #[graphql(description = "The body section of a block")]
    ordered_tx_hashes: Vec<Hash>,
    #[graphql(description = "Hash of the block")]
    hash:              Hash,
}

#[derive(juniper::GraphQLObject, Clone)]
#[graphql(description = "A block header is like the metadata of a block.")]
pub struct BlockHeader {
    #[graphql(
        description = "Identifier of a chain in order to prevent replay attacks across channels "
    )]
    pub chain_id:                       Hash,
    #[graphql(description = "block height")]
    pub height:                         Uint64,
    #[graphql(description = "The height to which the block has been executed")]
    pub exec_height:                    Uint64,
    #[graphql(description = "The hash of the serialized previous block")]
    pub prev_hash:                      Hash,
    #[graphql(description = "A timestamp that records when the block was created")]
    pub timestamp:                      Uint64,
    #[graphql(description = "The merkle root of ordered transactions")]
    pub order_root:                     MerkleRoot,
    #[graphql(description = "The hash of ordered signed transactions")]
    pub order_signed_transactions_hash: Hash,
    #[graphql(description = "The merkle roots of all the confirms")]
    pub confirm_root:                   Vec<MerkleRoot>,
    #[graphql(description = "The merkle root of state root")]
    pub state_root:                     MerkleRoot,
    #[graphql(description = "The merkle roots of receipts")]
    pub receipt_root:                   Vec<MerkleRoot>,
    #[graphql(description = "The sum of all transactions costs")]
    pub cycles_used:                    Vec<Uint64>,
    #[graphql(description = "The address descirbed who packed the block")]
    pub proposer:                       Address,
    pub proof:                          Proof,
    #[graphql(description = "The version of validator is designed for cross chain")]
    pub validator_version:              Uint64,
    pub validators:                     Vec<Validator>,
}

#[derive(juniper::GraphQLObject, Clone)]
#[graphql(description = "The verifier of the block header proved")]
pub struct Proof {
    pub height:     Uint64,
    pub round:      Uint64,
    pub block_hash: Hash,
    pub signature:  Bytes,
    pub bitmap:     Bytes,
}

#[derive(juniper::GraphQLObject, Clone)]
#[graphql(description = "Validator address set")]
pub struct Validator {
    pub address:        Address,
    pub propose_weight: i32,
    pub vote_weight:    i32,
}

impl From<protocol::types::BlockHeader> for BlockHeader {
    fn from(block_header: protocol::types::BlockHeader) -> Self {
        BlockHeader {
            chain_id:                       Hash::from(block_header.chain_id),
            height:                         Uint64::from(block_header.height),
            exec_height:                    Uint64::from(block_header.exec_height),
            prev_hash:                      Hash::from(block_header.prev_hash),
            timestamp:                      Uint64::from(block_header.timestamp),
            order_root:                     MerkleRoot::from(block_header.order_root),
            order_signed_transactions_hash: Hash::from(block_header.order_signed_transactions_hash),
            state_root:                     MerkleRoot::from(block_header.state_root),
            confirm_root:                   block_header
                .confirm_root
                .into_iter()
                .map(MerkleRoot::from)
                .collect(),
            receipt_root:                   block_header
                .receipt_root
                .into_iter()
                .map(MerkleRoot::from)
                .collect(),
            cycles_used:                    block_header
                .cycles_used
                .into_iter()
                .map(Uint64::from)
                .collect(),
            proposer:                       Address::from(block_header.proposer),
            proof:                          Proof::from(block_header.proof),
            validator_version:              Uint64::from(block_header.validator_version),
            validators:                     block_header
                .validators
                .into_iter()
                .map(Validator::from)
                .collect(),
        }
    }
}

impl From<protocol::types::Block> for Block {
    fn from(block: protocol::types::Block) -> Self {
        Block {
            header:            BlockHeader::from(block.header.clone()),
            ordered_tx_hashes: block
                .ordered_tx_hashes
                .clone()
                .into_iter()
                .map(MerkleRoot::from)
                .collect(),
            hash:              Hash::from(PHash::digest(
                block.header.encode_fixed().expect("rlp encode never fail"),
            )),
        }
    }
}

impl From<protocol::types::Proof> for Proof {
    fn from(proof: protocol::types::Proof) -> Self {
        Proof {
            height:     Uint64::from(proof.height),
            round:      Uint64::from(proof.round),
            block_hash: Hash::from(proof.block_hash),
            signature:  Bytes::from(proof.signature),
            bitmap:     Bytes::from(proof.bitmap),
        }
    }
}

impl From<protocol::types::Validator> for Validator {
    fn from(validator: protocol::types::Validator) -> Self {
        Validator {
            address:        Address::from(validator.address),
            propose_weight: validator.vote_weight as i32,
            vote_weight:    validator.vote_weight as i32,
        }
    }
}
