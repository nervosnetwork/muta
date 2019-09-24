use crate::schema::{Address, Fee, Hash, MerkleRoot, Uint64};

#[derive(GraphQLObject, Clone)]
#[graphql(description = "Epoch")]
pub struct Epoch {
    header:            EpochHeader,
    ordered_tx_hashes: Vec<Hash>,
}

#[derive(GraphQLObject, Clone)]
#[graphql(description = "Epoch header")]
pub struct EpochHeader {
    pub chain_id:     Hash,
    pub epoch_id:     Uint64,
    pub pre_hash:     Hash,
    pub timestamp:    Uint64,
    pub order_root:   MerkleRoot,
    pub confirm_root: Vec<MerkleRoot>,
    pub state_root:   MerkleRoot,
    pub receipt_root: Vec<MerkleRoot>,
    pub cycles_used:  Vec<Fee>,
    pub proposer:     Address,
    // proof:             Proof,
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
            cycles_used:       epoch_header
                .cycles_used
                .into_iter()
                .map(Fee::from)
                .collect(),
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
