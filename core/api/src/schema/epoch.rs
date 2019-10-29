use crate::schema::{Address, Hash, MerkleRoot, Uint64};

#[derive(GraphQLObject, Clone)]
#[graphql(
    description = "Epoch is a single digital record created within a blockchain. \
                   Each epoch contains a record of the previous Epoch, \
                   and when linked together these become the “chain”.\
                   An epoch is always composed of header and body."
)]
pub struct Epoch {
    #[graphql(description = "The header section of an epoch")]
    header: EpochHeader,
    #[graphql(description = "The body section of an epoch")]
    ordered_tx_hashes: Vec<Hash>,
}

#[derive(GraphQLObject, Clone)]
#[graphql(description = "An epoch header is like the metadata of an epoch.")]
pub struct EpochHeader {
    #[graphql(
        description = "Identifier of a chain in order to prevent replay attacks across channels "
    )]
    pub chain_id: Hash,
    #[graphql(description = "Known as the block height like other blockchain")]
    pub epoch_id: Uint64,
    #[graphql(description = "The hash of the serialized previous epoch")]
    pub pre_hash: Hash,
    #[graphql(description = "A timestamp that records when the epoch was created")]
    pub timestamp: Uint64,
    #[graphql(description = "The merkle root of ordered transactions")]
    pub order_root: MerkleRoot,
    #[graphql(description = "The merkle roots of all the confirms")]
    pub confirm_root: Vec<MerkleRoot>,
    #[graphql(description = "The merkle root of state root")]
    pub state_root: MerkleRoot,
    #[graphql(description = "The merkle roots of receipts")]
    pub receipt_root: Vec<MerkleRoot>,
    #[graphql(description = "The sum of all transactions costs")]
    pub cycles_used: Uint64,
    #[graphql(description = "The address descirbed who packed the epoch")]
    pub proposer: Address,
    // proof:             Proof,
    #[graphql(description = "The version of validator is designed for cross chain")]
    pub validator_version: Uint64,
    // validators:        Vec<Validator>,
}

impl From<protocol::types::EpochHeader> for EpochHeader {
    fn from(epoch_header: protocol::types::EpochHeader) -> Self {
        EpochHeader {
            chain_id:          Hash::from(epoch_header.chain_id),
            epoch_id:          Uint64::from(epoch_header.epoch_id),
            pre_hash:          Hash::from(epoch_header.pre_hash),
            timestamp:         Uint64::from(epoch_header.timestamp),
            order_root:        MerkleRoot::from(epoch_header.order_root),
            confirm_root:      epoch_header
                .confirm_root
                .into_iter()
                .map(MerkleRoot::from)
                .collect(),
            state_root:        MerkleRoot::from(epoch_header.state_root),
            receipt_root:      epoch_header
                .receipt_root
                .into_iter()
                .map(MerkleRoot::from)
                .collect(),
            cycles_used:       Uint64::from(epoch_header.cycles_used),
            proposer:          Address::from(protocol::types::Address::User(epoch_header.proposer)),
            validator_version: Uint64::from(epoch_header.validator_version),
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
