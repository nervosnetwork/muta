use crate::schema::{Address, Bytes, Hash, MerkleRoot, Uint64};

#[derive(GraphQLObject, Clone)]
#[graphql(
    description = "Epoch is a single digital record created within a blockchain. \
                   Each epoch contains a record of the previous Epoch, \
                   and when linked together these become the “chain”.\
                   An epoch is always composed of header and body."
)]
pub struct Epoch {
    #[graphql(description = "The header section of an epoch")]
    header:            EpochHeader,
    #[graphql(description = "The body section of an epoch")]
    ordered_tx_hashes: Vec<Hash>,
}

#[derive(GraphQLObject, Clone)]
#[graphql(description = "An epoch header is like the metadata of an epoch.")]
pub struct EpochHeader {
    #[graphql(
        description = "Identifier of a chain in order to prevent replay attacks across channels "
    )]
    pub chain_id:          Hash,
    #[graphql(description = "Known as the block height like other blockchain")]
    pub epoch_id:          Uint64,
    #[graphql(description = "The hash of the serialized previous epoch")]
    pub pre_hash:          Hash,
    #[graphql(description = "A timestamp that records when the epoch was created")]
    pub timestamp:         Uint64,
    #[graphql(description = "The merkle root of ordered transactions")]
    pub order_root:        MerkleRoot,
    #[graphql(description = "The merkle roots of all the confirms")]
    pub confirm_root:      Vec<MerkleRoot>,
    #[graphql(description = "The merkle root of state root")]
    pub state_root:        MerkleRoot,
    #[graphql(description = "The merkle roots of receipts")]
    pub receipt_root:      Vec<MerkleRoot>,
    #[graphql(description = "The sum of all transactions costs")]
    pub cycles_used:       Vec<Uint64>,
    #[graphql(description = "The address descirbed who packed the epoch")]
    pub proposer:          Address,
    pub proof:             Proof,
    #[graphql(description = "The version of validator is designed for cross chain")]
    pub validator_version: Uint64,
    pub validators:        Vec<Validator>,
}

#[derive(GraphQLObject, Clone)]
#[graphql(description = "The verifier of the epoch header proved")]
pub struct Proof {
    pub epoch_id:   Uint64,
    pub round:      Uint64,
    pub epoch_hash: Hash,
    pub signature:  Bytes,
    pub bitmap:     Bytes,
}

#[derive(GraphQLObject, Clone)]
#[graphql(description = "Validator address set")]
pub struct Validator {
    pub address:        Address,
    pub propose_weight: i32,
    pub vote_weight:    i32,
}

impl From<protocol::types::EpochHeader> for EpochHeader {
    fn from(epoch_header: protocol::types::EpochHeader) -> Self {
        EpochHeader {
            chain_id:          Hash::from(epoch_header.chain_id),
            epoch_id:          Uint64::from(epoch_header.epoch_id),
            pre_hash:          Hash::from(epoch_header.pre_hash),
            timestamp:         Uint64::from(epoch_header.timestamp),
            order_root:        MerkleRoot::from(epoch_header.order_root),
            state_root:        MerkleRoot::from(epoch_header.state_root),
            confirm_root:      epoch_header
                .confirm_root
                .into_iter()
                .map(MerkleRoot::from)
                .collect(),
            receipt_root:      epoch_header
                .receipt_root
                .into_iter()
                .map(MerkleRoot::from)
                .collect(),
            cycles_used:       epoch_header
                .cycles_used
                .into_iter()
                .map(Uint64::from)
                .collect(),
            proposer:          Address::from(epoch_header.proposer),
            proof:             Proof::from(epoch_header.proof),
            validator_version: Uint64::from(epoch_header.validator_version),
            validators:        epoch_header
                .validators
                .into_iter()
                .map(Validator::from)
                .collect(),
        }
    }
}

impl From<protocol::types::Epoch> for Epoch {
    fn from(epoch: protocol::types::Epoch) -> Self {
        Epoch {
            header:            EpochHeader::from(epoch.header),
            ordered_tx_hashes: epoch
                .ordered_tx_hashes
                .into_iter()
                .map(MerkleRoot::from)
                .collect(),
        }
    }
}

impl From<protocol::types::Proof> for Proof {
    fn from(proof: protocol::types::Proof) -> Self {
        Proof {
            epoch_id:   Uint64::from(proof.epoch_id),
            round:      Uint64::from(proof.round),
            epoch_hash: Hash::from(proof.epoch_hash),
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
