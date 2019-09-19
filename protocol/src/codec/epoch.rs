use std::convert::TryFrom;

use bytes::Bytes;
use prost::Message;

use crate::{
    codec::{
        primitive::{Hash, UserAddress},
        CodecError, ProtocolCodecSync,
    },
    field, impl_default_bytes_codec_for,
    types::primitive as protocol_primitive,
    types::Bloom,
    ProtocolError, ProtocolResult,
};

// #####################
// Protobuf
// #####################

#[derive(Clone, Message)]
pub struct Epoch {
    #[prost(message, tag = "1")]
    pub header: Option<EpochHeader>,

    #[prost(message, repeated, tag = "2")]
    pub ordered_tx_hashes: Vec<Hash>,
}

#[derive(Clone, Message)]
pub struct EpochHeader {
    #[prost(message, tag = "1")]
    pub chain_id: Option<Hash>,

    #[prost(uint64, tag = "2")]
    pub epoch_id: u64,

    #[prost(message, tag = "3")]
    pub pre_hash: Option<Hash>,

    #[prost(uint64, tag = "4")]
    pub timestamp: u64,

    #[prost(bytes, tag = "5")]
    pub logs_bloom: Vec<u8>,

    #[prost(message, tag = "6")]
    pub order_root: Option<Hash>,

    #[prost(message, repeated, tag = "7")]
    pub confirm_root: Vec<Hash>,

    #[prost(message, tag = "8")]
    pub state_root: Option<Hash>,

    #[prost(message, repeated, tag = "9")]
    pub receipt_root: Vec<Hash>,

    #[prost(uint64, tag = "10")]
    pub cycles_used: u64,

    #[prost(message, tag = "11")]
    pub proposer: Option<UserAddress>,

    #[prost(message, tag = "12")]
    pub proof: Option<Proof>,

    #[prost(uint64, tag = "13")]
    pub validator_version: u64,

    #[prost(message, repeated, tag = "14")]
    pub validators: Vec<Validator>,
}

#[derive(Clone, Message)]
pub struct Proof {
    #[prost(uint64, tag = "1")]
    pub epoch_id: u64,

    #[prost(uint64, tag = "2")]
    pub round: u64,

    #[prost(message, tag = "3")]
    pub epoch_hash: Option<Hash>,

    #[prost(bytes, tag = "4")]
    pub signature: Vec<u8>,
}

#[derive(Clone, Message)]
pub struct Validator {
    #[prost(message, tag = "1")]
    pub address: Option<UserAddress>,

    #[prost(uint64, tag = "2")]
    pub weight: u64,
}

#[derive(Clone, Message)]
pub struct Pill {
    #[prost(message, tag = "1")]
    pub epoch: Option<Epoch>,

    #[prost(message, repeated, tag = "2")]
    pub propose_hashes: Vec<Hash>,
}

#[derive(Clone, Message)]
pub struct EpochId {
    #[prost(uint64, tag = "1")]
    pub id: u64,
}

// #################
// Conversion
// #################

// Epoch

impl From<epoch::Epoch> for Epoch {
    fn from(epoch: epoch::Epoch) -> Epoch {
        let header = Some(EpochHeader::from(epoch.header));
        let ordered_tx_hashes = epoch
            .ordered_tx_hashes
            .into_iter()
            .map(Hash::from)
            .collect::<Vec<_>>();

        Epoch {
            header,
            ordered_tx_hashes,
        }
    }
}

impl TryFrom<Epoch> for epoch::Epoch {
    type Error = ProtocolError;

    fn try_from(epoch: Epoch) -> Result<epoch::Epoch, Self::Error> {
        let header = field!(epoch.header, "Epoch", "header")?;

        let mut ordered_tx_hashes = Vec::new();
        for hash in epoch.ordered_tx_hashes {
            ordered_tx_hashes.push(protocol_primitive::Hash::try_from(hash)?);
        }

        let epoch = epoch::Epoch {
            header: epoch::EpochHeader::try_from(header)?,
            ordered_tx_hashes,
        };

        Ok(epoch)
    }
}

// EpochHeader

impl From<epoch::EpochHeader> for EpochHeader {
    fn from(epoch_header: epoch::EpochHeader) -> EpochHeader {
        let chain_id = Some(Hash::from(epoch_header.chain_id));
        let pre_hash = Some(Hash::from(epoch_header.pre_hash));
        let order_root = Some(Hash::from(epoch_header.order_root));
        let state_root = Some(Hash::from(epoch_header.state_root));
        let proposer = Some(UserAddress::from(epoch_header.proposer));
        let proof = Some(Proof::from(epoch_header.proof));

        let confirm_root = epoch_header
            .confirm_root
            .into_iter()
            .map(Hash::from)
            .collect::<Vec<_>>();
        let receipt_root = epoch_header
            .receipt_root
            .into_iter()
            .map(Hash::from)
            .collect::<Vec<_>>();
        let validators = epoch_header
            .validators
            .into_iter()
            .map(Validator::from)
            .collect::<Vec<_>>();

        EpochHeader {
            chain_id,
            epoch_id: epoch_header.epoch_id,
            pre_hash,
            timestamp: epoch_header.timestamp,
            logs_bloom: epoch_header.logs_bloom.as_bytes().to_vec(),
            order_root,
            confirm_root,
            state_root,
            receipt_root,
            cycles_used: epoch_header.cycles_used,
            proposer,
            proof,
            validator_version: epoch_header.validator_version,
            validators,
        }
    }
}

impl TryFrom<EpochHeader> for epoch::EpochHeader {
    type Error = ProtocolError;

    fn try_from(epoch_header: EpochHeader) -> Result<epoch::EpochHeader, Self::Error> {
        let chain_id = field!(epoch_header.chain_id, "EpochHeader", "chain_id")?;
        let pre_hash = field!(epoch_header.pre_hash, "EpochHeader", "pre_hash")?;
        let order_root = field!(epoch_header.order_root, "EpochHeader", "order_root")?;
        let state_root = field!(epoch_header.state_root, "EpochHeader", "state_root")?;
        let proposer = field!(epoch_header.proposer, "EpochHeader", "proposer")?;
        let proof = field!(epoch_header.proof, "EpochHeader", "proof")?;

        let mut confirm_root = Vec::new();
        for root in epoch_header.confirm_root {
            confirm_root.push(protocol_primitive::Hash::try_from(root)?);
        }

        let mut receipt_root = Vec::new();
        for root in epoch_header.receipt_root {
            receipt_root.push(protocol_primitive::Hash::try_from(root)?);
        }

        let mut validators = Vec::new();
        for validator in epoch_header.validators {
            validators.push(epoch::Validator::try_from(validator)?);
        }

        let proof = epoch::EpochHeader {
            chain_id: protocol_primitive::Hash::try_from(chain_id)?,
            epoch_id: epoch_header.epoch_id,
            pre_hash: protocol_primitive::Hash::try_from(pre_hash)?,
            timestamp: epoch_header.timestamp,
            logs_bloom: Bloom::from_slice(&epoch_header.logs_bloom),
            order_root: protocol_primitive::Hash::try_from(order_root)?,
            confirm_root,
            state_root: protocol_primitive::Hash::try_from(state_root)?,
            receipt_root,
            cycles_used: epoch_header.cycles_used,
            proposer: protocol_primitive::UserAddress::try_from(proposer)?,
            proof: epoch::Proof::try_from(proof)?,
            validator_version: epoch_header.validator_version,
            validators,
        };

        Ok(proof)
    }
}

// Proof

impl From<epoch::Proof> for Proof {
    fn from(proof: epoch::Proof) -> Proof {
        let epoch_hash = Some(Hash::from(proof.epoch_hash));

        Proof {
            epoch_id: proof.epoch_id,
            round: proof.round,
            epoch_hash,
            signature: proof.signature.to_vec(),
        }
    }
}

impl TryFrom<Proof> for epoch::Proof {
    type Error = ProtocolError;

    fn try_from(proof: Proof) -> Result<epoch::Proof, Self::Error> {
        let epoch_hash = field!(proof.epoch_hash, "Proof", "epoch_hash")?;

        let proof = epoch::Proof {
            epoch_id:   proof.epoch_id,
            round:      proof.round,
            epoch_hash: protocol_primitive::Hash::try_from(epoch_hash)?,
            signature:  Bytes::from(proof.signature),
        };

        Ok(proof)
    }
}

// Validator

impl From<epoch::Validator> for Validator {
    fn from(validator: epoch::Validator) -> Validator {
        let address = Some(UserAddress::from(validator.address));

        Validator {
            address,
            weight: validator.weight,
        }
    }
}

impl TryFrom<Validator> for epoch::Validator {
    type Error = ProtocolError;

    fn try_from(validator: Validator) -> Result<epoch::Validator, Self::Error> {
        let address = field!(validator.address, "Validator", "address")?;

        let validator = epoch::Validator {
            address: protocol_primitive::UserAddress::try_from(address)?,
            weight:  validator.weight,
        };

        Ok(validator)
    }
}

// Pill

impl From<epoch::Pill> for Pill {
    fn from(pill: epoch::Pill) -> Pill {
        let epoch = Some(Epoch::from(pill.epoch));
        let propose_hashes = pill
            .propose_hashes
            .into_iter()
            .map(Hash::from)
            .collect::<Vec<_>>();

        Pill {
            epoch,
            propose_hashes,
        }
    }
}

impl TryFrom<Pill> for epoch::Pill {
    type Error = ProtocolError;

    fn try_from(pill: Pill) -> Result<epoch::Pill, Self::Error> {
        let epoch = field!(pill.epoch, "Pill", "epoch")?;

        let mut propose_hashes = Vec::new();
        for hash in pill.propose_hashes {
            propose_hashes.push(protocol_primitive::Hash::try_from(hash)?);
        }

        let pill = epoch::Pill {
            epoch: epoch::Epoch::try_from(epoch)?,
            propose_hashes,
        };

        Ok(pill)
    }
}

// EpochId

impl From<epoch::EpochId> for EpochId {
    fn from(epoch_id: epoch::EpochId) -> EpochId {
        EpochId { id: epoch_id.id }
    }
}

impl TryFrom<EpochId> for epoch::EpochId {
    type Error = ProtocolError;

    fn try_from(epoch_id: EpochId) -> Result<epoch::EpochId, Self::Error> {
        let epoch_id = epoch::EpochId { id: epoch_id.id };
        Ok(epoch_id)
    }
}

// #################
// Codec
// #################

impl_default_bytes_codec_for!(epoch, [Epoch, EpochHeader, Proof, Validator, Pill, EpochId]);
