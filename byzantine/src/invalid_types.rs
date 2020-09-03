use std::error::Error;
use std::sync::Arc;

use bytes::Bytes;
use derive_more::Constructor;
use overlord::types::SignedProposal;
use overlord::{Codec, Crypto};
use rlp::Encodable;

use common_crypto::Secp256k1PrivateKey;
use core_consensus::util::OverlordCrypto;
use core_mempool::MsgPullTxs;
use protocol::traits::MessageCodec;
use protocol::types::{Address, Hash, Metadata, SignedTransaction, Validator};
use protocol::ProtocolResult;

use crate::utils::{
    gen_invalid_address, gen_invalid_aggregate_sig, gen_invalid_chain_id,
    gen_invalid_content_struct_proposal, gen_invalid_from, gen_invalid_hash, gen_invalid_lock,
    gen_invalid_proof, gen_invalid_request, gen_invalid_sig, gen_invalid_validators,
    gen_positive_range, gen_random_bytes, gen_range, gen_signed_proposal_from_header,
    gen_signed_tx, gen_valid_block, gen_valid_block_header, gen_valid_choke, gen_valid_hash,
    gen_valid_proposal, gen_valid_qc, gen_valid_raw_tx, gen_valid_signed_choke,
    gen_valid_signed_proposal, gen_valid_signed_tx, gen_valid_signed_vote, gen_valid_vote,
};
use crate::worker::State;

#[derive(Constructor, Clone, Debug, Eq, PartialEq)]
pub struct InvalidStruct {
    pub inner: Bytes,
}

impl InvalidStruct {
    pub fn gen(len: usize) -> Self {
        InvalidStruct {
            inner: gen_random_bytes(len),
        }
    }
}

impl MessageCodec for InvalidStruct {
    fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(self.inner.clone())
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(InvalidStruct::new(bytes))
    }
}

impl Codec for InvalidStruct {
    fn encode(&self) -> Result<Bytes, Box<dyn Error + Send>> {
        let bytes = self.inner.clone();
        Ok(bytes)
    }

    fn decode(data: Bytes) -> Result<Self, Box<dyn Error + Send>> {
        Ok(InvalidStruct::new(data))
    }
}

//################################
//##########  NewChoke  ##########
//########## ######################
pub fn gen_invalid_struct_new_choke(
    _state: &State,
    _crypto: &Arc<OverlordCrypto>,
    _my_pub_key: &Bytes,
) -> Vec<u8> {
    gen_random_bytes(100).to_vec()
}

pub fn gen_invalid_height_new_choke(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let mut choke = gen_valid_choke(state, my_pub_key);
    choke.height = gen_positive_range(state.height, 20);
    let signed_choke = gen_valid_signed_choke(choke, crypto, my_pub_key);
    signed_choke.rlp_bytes()
}

pub fn gen_invalid_round_new_choke(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let mut choke = gen_valid_choke(state, my_pub_key);
    choke.round = gen_positive_range(state.round, 20);
    let signed_choke = gen_valid_signed_choke(choke, crypto, my_pub_key);
    signed_choke.rlp_bytes()
}

pub fn gen_invalid_from_new_vote(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let mut choke = gen_valid_choke(state, my_pub_key);
    choke.from = gen_invalid_from();
    let signed_choke = gen_valid_signed_choke(choke, crypto, my_pub_key);
    signed_choke.rlp_bytes()
}

pub fn gen_invalid_sig_new_choke(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let choke = gen_valid_choke(state, my_pub_key);
    let mut signed_choke = gen_valid_signed_choke(choke, crypto, my_pub_key);
    signed_choke.signature = gen_invalid_sig();
    signed_choke.rlp_bytes()
}

pub fn gen_invalid_address_new_choke(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let choke = gen_valid_choke(state, my_pub_key);
    let mut signed_choke = gen_valid_signed_choke(choke, crypto, my_pub_key);
    signed_choke.address = gen_invalid_address().as_bytes();
    signed_choke.rlp_bytes()
}

//#############################
//##########  NewQC  ##########
//########## ###################
pub fn gen_invalid_struct_new_qc(_state: &State, _my_pub_key: &Bytes) -> Vec<u8> {
    gen_random_bytes(100).to_vec()
}

pub fn gen_invalid_height_new_qc(state: &State, my_pub_key: &Bytes) -> Vec<u8> {
    let mut qc = gen_valid_qc(state, my_pub_key);
    qc.height = gen_positive_range(state.height, 20);
    qc.rlp_bytes()
}

pub fn gen_invalid_round_new_qc(state: &State, my_pub_key: &Bytes) -> Vec<u8> {
    let mut qc = gen_valid_qc(state, my_pub_key);
    qc.round = gen_positive_range(state.round, 20);
    qc.rlp_bytes()
}

pub fn gen_invalid_block_hash_new_qc(state: &State, my_pub_key: &Bytes) -> Vec<u8> {
    let mut qc = gen_valid_qc(state, my_pub_key);
    qc.block_hash = gen_invalid_hash().as_bytes();
    qc.rlp_bytes()
}

pub fn gen_invalid_sig_new_qc(state: &State, my_pub_key: &Bytes) -> Vec<u8> {
    let mut qc = gen_valid_qc(state, my_pub_key);
    qc.signature = gen_invalid_aggregate_sig();
    qc.rlp_bytes()
}

pub fn gen_invalid_leader_new_qc(state: &State, my_pub_key: &Bytes) -> Vec<u8> {
    let mut qc = gen_valid_qc(state, my_pub_key);
    qc.leader = gen_invalid_address().as_bytes();
    qc.rlp_bytes()
}

//###############################
//##########  NewVote  ##########
//########## #####################
pub fn gen_invalid_struct_new_vote(
    _state: &State,
    _crypto: &Arc<OverlordCrypto>,
    _my_pub_key: &Bytes,
) -> Vec<u8> {
    gen_random_bytes(100).to_vec()
}

pub fn gen_invalid_height_new_vote(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let mut vote = gen_valid_vote(state);
    vote.height = gen_positive_range(state.height, 20);
    let signed_vote = gen_valid_signed_vote(vote, crypto, my_pub_key);
    signed_vote.rlp_bytes()
}

pub fn gen_invalid_round_new_vote(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let mut vote = gen_valid_vote(state);
    vote.round = gen_positive_range(state.round, 20);
    let signed_vote = gen_valid_signed_vote(vote, crypto, my_pub_key);
    signed_vote.rlp_bytes()
}

pub fn gen_invalid_block_hash_new_vote(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let mut vote = gen_valid_vote(state);
    vote.block_hash = gen_invalid_hash().as_bytes();
    let signed_vote = gen_valid_signed_vote(vote, crypto, my_pub_key);
    signed_vote.rlp_bytes()
}

pub fn gen_invalid_sig_new_vote(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let vote = gen_valid_vote(state);
    let mut signed_vote = gen_valid_signed_vote(vote, crypto, my_pub_key);
    signed_vote.signature = gen_invalid_sig();
    signed_vote.rlp_bytes()
}

pub fn gen_invalid_voter_new_vote(
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let vote = gen_valid_vote(state);
    let mut signed_vote = gen_valid_signed_vote(vote, crypto, my_pub_key);
    signed_vote.voter = gen_random_bytes(100);
    signed_vote.rlp_bytes()
}

//###################################
//##########  NewProposal  ##########
//########## #########################
pub fn gen_valid_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let order_tx_hashes: Vec<Hash> = (0..gen_range(0, 1000)).map(|_| gen_valid_hash()).collect();
    let propose_tx_hashes: Vec<Hash> = (0..gen_range(0, 1000)).map(|_| gen_valid_hash()).collect();
    let header = gen_valid_block_header(
        state,
        metadata,
        my_address,
        validators,
        order_tx_hashes.clone(),
    );

    let block = gen_valid_block(header, order_tx_hashes);
    let proposal = gen_valid_proposal(block, state, my_pub_key, propose_tx_hashes);
    let signed_proposal = gen_valid_signed_proposal(proposal, crypto);
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_prop_proposer_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    let block = gen_valid_block(header, vec![]);
    let mut proposal = gen_valid_proposal(block, state, my_pub_key, vec![]);
    proposal.proposer = gen_invalid_address().as_bytes();
    let signed_proposal = gen_valid_signed_proposal(proposal, crypto);
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_lock_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    let block = gen_valid_block(header, vec![]);
    let mut proposal = gen_valid_proposal(block, state, my_pub_key, vec![]);
    proposal.lock = Some(gen_invalid_lock());
    let signed_proposal = gen_valid_signed_proposal(proposal, crypto);
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_block_hash_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    let block = gen_valid_block(header, vec![]);
    let mut proposal = gen_valid_proposal(block, state, my_pub_key, vec![]);
    proposal.block_hash = gen_invalid_hash().as_bytes();
    let signed_proposal = gen_valid_signed_proposal(proposal, crypto);
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_content_struct_new_proposal(
    state: &State,
    _metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    _my_address: &Address,
    my_pub_key: &Bytes,
    _validators: &[Validator],
) -> Vec<u8> {
    let proposal = gen_invalid_content_struct_proposal(state, my_pub_key);
    let signature = crypto
        .sign(crypto.hash(proposal.content.inner.clone()))
        .expect("sign proposal failed");

    let signed_proposal = SignedProposal {
        signature,
        proposal,
    };
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_round_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    let block = gen_valid_block(header, vec![]);
    let mut proposal = gen_valid_proposal(block, state, my_pub_key, vec![]);
    proposal.round = gen_positive_range(state.round, 20);
    let signed_proposal = gen_valid_signed_proposal(proposal, crypto);
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_prop_height_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    let block = gen_valid_block(header, vec![]);
    let mut proposal = gen_valid_proposal(block, state, my_pub_key, vec![]);
    proposal.height = gen_positive_range(state.height, 20);
    let signed_proposal = gen_valid_signed_proposal(proposal, crypto);
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_sig_new_proposal(
    state: &State,
    metadata: &Metadata,
    _crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    let block = gen_valid_block(header, vec![]);
    let proposal = gen_valid_proposal(block, state, my_pub_key, vec![]);
    let signed_proposal = SignedProposal {
        proposal,
        signature: gen_invalid_sig(),
    };
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_tx_hash_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let order_tx_hashes: Vec<Hash> = (0..gen_range(0, 1000))
        .map(|_| gen_invalid_hash())
        .collect();
    let propose_tx_hashes: Vec<Hash> = (0..gen_range(0, 1000))
        .map(|_| gen_invalid_hash())
        .collect();
    let header = gen_valid_block_header(
        state,
        metadata,
        my_address,
        validators,
        order_tx_hashes.clone(),
    );

    let block = gen_valid_block(header, order_tx_hashes);
    let proposal = gen_valid_proposal(block, state, my_pub_key, propose_tx_hashes);
    let signed_proposal = gen_valid_signed_proposal(proposal, crypto);
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_validators_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.validators = gen_invalid_validators();
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_version_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.validator_version = gen_range(u64::MIN, u64::MAX);
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_proof_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.proof = gen_invalid_proof();
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_block_proposer_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.proposer = gen_invalid_address();
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_cycle_used_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.cycles_used = vec![gen_range(u64::MIN, u64::MAX)];
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_receipt_root_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.receipt_root = vec![gen_invalid_hash()];
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_state_root_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.state_root = gen_invalid_hash();
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_confirm_root_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.confirm_root = vec![gen_invalid_hash()];
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_signed_tx_hash_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.order_signed_transactions_hash = gen_invalid_hash();
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_order_root_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.order_root = gen_invalid_hash();
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_timestamp_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.timestamp = gen_positive_range(state.prev_timestamp, 1_000_000);
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_exec_height_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.exec_height = gen_positive_range(state.exec_height, 20);
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_height_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.height = gen_positive_range(state.height, 20);
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_prev_hash_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.prev_hash = gen_invalid_hash();
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_chain_id_new_proposal(
    state: &State,
    metadata: &Metadata,
    crypto: &Arc<OverlordCrypto>,
    my_address: &Address,
    my_pub_key: &Bytes,
    validators: &[Validator],
) -> Vec<u8> {
    let mut header = gen_valid_block_header(state, metadata, my_address, validators, vec![]);
    header.chain_id = gen_invalid_chain_id();
    gen_signed_proposal_from_header(header, state, crypto, my_pub_key)
}

pub fn gen_invalid_struct_new_proposal(
    _state: &State,
    _metadata: &Metadata,
    _crypto: &Arc<OverlordCrypto>,
    _my_address: &Address,
    _my_pub_key: &Bytes,
    _validators: &[Validator],
) -> Vec<u8> {
    gen_random_bytes(1000).to_vec()
}

//###############################
//##########  PullTxs  ##########
//########## #####################
pub fn gen_invalid_height_pull_txs(height: u64) -> MsgPullTxs {
    let tx_num = gen_positive_range(100, 300);
    let tx_hashes: Vec<Hash> = (0..tx_num).map(|_| gen_valid_hash()).collect();
    MsgPullTxs {
        height: Some(gen_positive_range(height, 100)),
        hashes: tx_hashes,
    }
}

pub fn gen_invalid_hash_pull_txs(_height: u64) -> MsgPullTxs {
    let tx_num = gen_positive_range(100, 300);
    let tx_hashes: Vec<Hash> = (0..tx_num).map(|_| gen_invalid_hash()).collect();
    MsgPullTxs {
        height: None,
        hashes: tx_hashes,
    }
}

pub fn gen_not_exists_txs_pull_txs(_height: u64) -> MsgPullTxs {
    let tx_num = gen_positive_range(100, 300);
    let tx_hashes: Vec<Hash> = (0..tx_num).map(|_| gen_valid_hash()).collect();
    MsgPullTxs {
        height: None,
        hashes: tx_hashes,
    }
}

//#############################
//##########  NewTx  ##########
//########## ###################
pub fn gen_invalid_hash_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> SignedTransaction {
    let raw = gen_valid_raw_tx(pri_key, height, metadata);
    gen_signed_tx(raw, pri_key, Some(gen_random_bytes(100)), None)
}

pub fn gen_invalid_sig_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> SignedTransaction {
    let raw = gen_valid_raw_tx(pri_key, height, metadata);
    gen_signed_tx(raw, pri_key, None, Some(gen_random_bytes(100)))
}

pub fn gen_invalid_chain_id_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> SignedTransaction {
    let mut raw = gen_valid_raw_tx(pri_key, height, metadata);
    raw.chain_id = gen_invalid_chain_id();
    gen_valid_signed_tx(raw, pri_key)
}

pub fn gen_invalid_cycles_price_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> SignedTransaction {
    let mut raw = gen_valid_raw_tx(pri_key, height, metadata);
    raw.cycles_price = gen_range(metadata.cycles_price + 1, u64::MAX);
    gen_valid_signed_tx(raw, pri_key)
}

pub fn gen_invalid_cycles_limit_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> SignedTransaction {
    let mut raw = gen_valid_raw_tx(pri_key, height, metadata);
    raw.cycles_limit = gen_range(metadata.cycles_limit + 1, u64::MAX);
    gen_valid_signed_tx(raw, pri_key)
}

pub fn gen_invalid_nonce_of_rand_len_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> SignedTransaction {
    let mut raw = gen_valid_raw_tx(pri_key, height, metadata);
    raw.nonce = gen_invalid_hash();
    gen_valid_signed_tx(raw, pri_key)
}

pub fn gen_invalid_nonce_dup_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
    nonce: Hash,
) -> SignedTransaction {
    let mut raw = gen_valid_raw_tx(pri_key, height, metadata);
    raw.nonce = nonce;
    gen_valid_signed_tx(raw, pri_key)
}

pub fn gen_invalid_request_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> SignedTransaction {
    let mut raw = gen_valid_raw_tx(pri_key, height, metadata);
    raw.request = gen_invalid_request();
    gen_valid_signed_tx(raw, pri_key)
}

pub fn gen_invalid_timeout_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> SignedTransaction {
    let mut raw = gen_valid_raw_tx(pri_key, height, metadata);
    raw.timeout = gen_positive_range(height + metadata.timeout_gap, 100);
    gen_valid_signed_tx(raw, pri_key)
}

pub fn gen_invalid_sender_signed_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> SignedTransaction {
    let mut raw = gen_valid_raw_tx(pri_key, height, metadata);
    raw.sender = gen_invalid_address();
    gen_valid_signed_tx(raw, pri_key)
}
