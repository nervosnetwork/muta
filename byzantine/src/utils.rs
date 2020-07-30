use std::convert::TryFrom;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use overlord::types::{
    AggregatedChoke, AggregatedSignature, AggregatedVote, Choke, PoLC, Proposal, SignedChoke,
    SignedProposal, SignedVote, UpdateFrom, Vote, VoteType,
};
use overlord::Crypto;
use rand::distributions::uniform::{SampleBorrow, SampleUniform};
use rand::distributions::Alphanumeric;
use rand::{random, Rng};
use rlp::{Encodable, RlpStream};

use common_crypto::{
    HashValue, PrivateKey, PublicKey, Secp256k1PrivateKey, Signature, ToPublicKey,
    UncompressedPublicKey,
};
use common_merkle::Merkle;
use core_consensus::fixed_types::FixedPill;
use core_consensus::util::OverlordCrypto;
use protocol::fixed_codec::FixedCodec;
use protocol::types::{
    Address, Block, BlockHeader, Hash, Metadata, Pill, Proof, RawTransaction, SignedTransaction,
    TransactionRequest, Validator,
};

use crate::invalid_types::InvalidStruct;
use crate::worker::State;

const VALIDATOR_VERSION: u64 = 0;
const HASH_LEN: u64 = 32;
const ADDRESS_LEN: u64 = 20;
const SIGNATURE_LEN: u64 = 192;
const BITMAP_LEN: u64 = 1;

pub fn time_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn gen_random_bytes(len: usize) -> Bytes {
    let vec = (0..len).map(|_| random::<u8>()).collect::<Vec<_>>();
    Bytes::from(vec)
}

pub fn gen_random_string(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .collect()
}

pub fn gen_range<T: SampleUniform, B1, B2>(low: B1, high: B2) -> T
where
    B1: SampleBorrow<T> + Sized,
    B2: SampleBorrow<T> + Sized,
{
    let mut rng = rand::thread_rng();
    rng.gen_range(low, high)
}

pub fn gen_bool(p: f64) -> bool {
    let mut rng = rand::thread_rng();
    if p >= 1.0 {
        true
    } else {
        rng.gen_bool(p)
    }
}

pub fn gen_valid_raw_tx(
    pri_key: &Secp256k1PrivateKey,
    height: u64,
    metadata: &Metadata,
) -> RawTransaction {
    RawTransaction {
        chain_id:     metadata.chain_id.clone(),
        cycles_price: gen_range(0, metadata.cycles_price),
        cycles_limit: gen_range(0, metadata.cycles_limit),
        nonce:        gen_valid_hash(),
        request:      gen_transfer_tx_request(),
        timeout:      gen_range(height, height + metadata.timeout_gap),
        sender:       gen_address_bytes(pri_key),
    }
}

pub fn gen_invalid_request() -> TransactionRequest {
    TransactionRequest {
        method:       gen_random_string(10),
        service_name: gen_random_string(10),
        payload:      gen_random_string(100),
    }
}

pub fn gen_transfer_tx_request() -> TransactionRequest {
    TransactionRequest {
        method: "asset".to_string(),
        service_name: "transfer".to_string(),
        payload: "{ \"asset_id\": \"0xf56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c\", \"to\":\"0x0000000000000000000000000000000000000001\", \"value\": 100 }".to_string(),
    }
}

pub fn gen_address_bytes(pri_key: &Secp256k1PrivateKey) -> Address {
    let pubkey = pri_key.pub_key();
    Address::from_pubkey_bytes(pubkey.to_uncompressed_bytes()).expect("get address failed")
}

pub fn gen_valid_hash() -> Hash {
    Hash::digest(gen_random_bytes(20))
}

pub fn gen_invalid_hash() -> Hash {
    let rand_len = gen_positive_range(HASH_LEN, 1);
    Hash::from_invalid_bytes(gen_random_bytes(rand_len as usize))
}

pub fn gen_invalid_address() -> Address {
    let rand_len = gen_positive_range(ADDRESS_LEN, 1);
    Address::from_invalid_bytes(gen_random_bytes(rand_len as usize))
}

pub fn gen_valid_signed_tx(
    raw: RawTransaction,
    pri_key: &Secp256k1PrivateKey,
) -> SignedTransaction {
    gen_signed_tx(raw, pri_key, None, None)
}

pub fn gen_signed_tx(
    raw: RawTransaction,
    pri_key: &Secp256k1PrivateKey,
    fixed_bytes: Option<Bytes>,
    sig: Option<Bytes>,
) -> SignedTransaction {
    let fixed_bytes =
        fixed_bytes.unwrap_or_else(|| raw.encode_fixed().expect("get bytes from raw_tx failed!"));
    let tx_hash = Hash::digest(fixed_bytes);
    let hash_value = HashValue::try_from(tx_hash.as_bytes().as_ref()).unwrap();
    let signature = sig.unwrap_or_else(|| pri_key.sign_message(&hash_value).to_bytes());
    let pubkey = pri_key.pub_key().to_bytes();
    SignedTransaction {
        raw,
        tx_hash,
        pubkey,
        signature,
    }
}

pub fn gen_valid_block_header(
    state: &State,
    metadata: &Metadata,
    my_address: &Address,
    validators: &[Validator],
    ordered_tx_hashes: Vec<Hash>,
) -> BlockHeader {
    let order_root = Merkle::from_hashes(ordered_tx_hashes).get_root_hash();
    BlockHeader {
        chain_id:                       metadata.chain_id.clone(),
        height:                         state.height,
        exec_height:                    state.exec_height,
        prev_hash:                      state.prev_hash.clone(),
        timestamp:                      time_now(),
        order_root:                     order_root.unwrap_or_else(Hash::from_empty),
        order_signed_transactions_hash: Hash::from_empty(),
        confirm_root:                   state.confirm_root.clone(),
        state_root:                     state.state_root.clone(),
        receipt_root:                   state.receipt_root.clone(),
        cycles_used:                    state.cycles_used.clone(),
        proposer:                       my_address.clone(),
        proof:                          state.proof.clone(),
        validator_version:              VALIDATOR_VERSION,
        validators:                     validators.to_vec(),
    }
}

pub fn gen_valid_block(header: BlockHeader, ordered_tx_hashes: Vec<Hash>) -> Block {
    Block {
        header,
        ordered_tx_hashes,
    }
}

pub fn gen_invalid_content_struct_proposal(
    state: &State,
    my_pub_key: &Bytes,
) -> Proposal<InvalidStruct> {
    let content = InvalidStruct::gen(1000);
    let hash = Hash::digest(content.inner.clone()).as_bytes();
    Proposal {
        height: state.height,
        round: state.round,
        content,
        block_hash: hash,
        lock: state.lock.clone(),
        proposer: my_pub_key.clone(),
    }
}

pub fn gen_valid_proposal(
    block: Block,
    state: &State,
    my_pub_key: &Bytes,
    propose_hashes: Vec<Hash>,
) -> Proposal<FixedPill> {
    let pill = Pill {
        block,
        propose_hashes,
    };
    let fixed_pill = FixedPill {
        inner: pill.clone(),
    };
    let hash = Hash::digest(
        pill.block
            .header
            .encode_fixed()
            .expect("encode block header failed"),
    )
    .as_bytes();
    Proposal {
        height:     state.height,
        round:      state.round,
        content:    fixed_pill,
        block_hash: hash,
        lock:       state.lock.clone(),
        proposer:   my_pub_key.clone(),
    }
}

pub fn gen_valid_signed_proposal(
    proposal: Proposal<FixedPill>,
    crypto: &Arc<OverlordCrypto>,
) -> SignedProposal<FixedPill> {
    let signature = crypto
        .sign(crypto.hash(Bytes::from(rlp::encode(&proposal))))
        .expect("sign proposal failed");

    SignedProposal {
        signature,
        proposal,
    }
}

pub fn gen_signed_proposal_from_header(
    header: BlockHeader,
    state: &State,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> Vec<u8> {
    let block = gen_valid_block(header, vec![]);
    let proposal = gen_valid_proposal(block, state, my_pub_key, vec![]);
    let signed_proposal = gen_valid_signed_proposal(proposal, crypto);
    signed_proposal.rlp_bytes()
}

pub fn gen_invalid_chain_id() -> Hash {
    Hash::digest(gen_random_bytes(20))
}

pub fn gen_positive_range(base: u64, range: u64) -> u64 {
    let low = if base < range { 0 } else { base - range };
    let high = if u64::MAX - base < range {
        u64::MAX
    } else {
        base + range
    };
    gen_range(low, high)
}

pub fn gen_invalid_sig() -> Bytes {
    gen_random_bytes(gen_positive_range(SIGNATURE_LEN, 1) as usize)
}

pub fn gen_invalid_proof() -> Proof {
    Proof {
        height:     gen_range(u64::MIN, u64::MAX),
        round:      gen_range(u64::MIN, u64::MAX),
        block_hash: gen_invalid_hash(),
        signature:  gen_invalid_sig(),
        bitmap:     gen_invalid_bitmap(),
    }
}

pub fn gen_invalid_bitmap() -> Bytes {
    gen_random_bytes(gen_positive_range(BITMAP_LEN, 1) as usize)
}

pub fn gen_invalid_validators() -> Vec<Validator> {
    (0..gen_range(0, 100))
        .map(|_| Validator {
            pub_key:        gen_random_bytes(32),
            propose_weight: gen_range(u32::MIN, u32::MAX),
            vote_weight:    gen_range(u32::MIN, u32::MAX),
        })
        .collect()
}

pub fn gen_invalid_lock() -> PoLC {
    PoLC {
        lock_round: gen_range(u64::MIN, u64::MAX),
        lock_votes: gen_invalid_qc(),
    }
}

pub fn gen_invalid_qc() -> AggregatedVote {
    AggregatedVote {
        signature:  gen_invalid_aggregate_sig(),
        vote_type:  gen_vote_type(),
        height:     gen_range(u64::MIN, u64::MAX),
        round:      gen_range(u64::MIN, u64::MAX),
        block_hash: gen_invalid_hash().as_bytes(),
        leader:     gen_invalid_address().as_bytes(),
    }
}

pub fn gen_invalid_aggregate_sig() -> AggregatedSignature {
    AggregatedSignature {
        signature:      gen_invalid_sig(),
        address_bitmap: gen_invalid_bitmap(),
    }
}

pub fn gen_valid_qc(state: &State, my_pub_key: &Bytes) -> AggregatedVote {
    AggregatedVote {
        signature:  gen_valid_aggregate_sig(),
        vote_type:  gen_vote_type(),
        height:     state.height,
        round:      state.round,
        block_hash: gen_invalid_hash().as_bytes(),
        leader:     my_pub_key.clone(),
    }
}

pub fn gen_valid_aggregate_sig() -> AggregatedSignature {
    AggregatedSignature {
        signature:      gen_random_bytes(SIGNATURE_LEN as usize),
        address_bitmap: gen_random_bytes(BITMAP_LEN as usize),
    }
}

pub fn gen_valid_signed_vote(
    vote: Vote,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> SignedVote {
    let signature = crypto
        .sign(crypto.hash(Bytes::from(rlp::encode(&vote))))
        .expect("sign proposal failed");

    SignedVote {
        signature,
        vote,
        voter: my_pub_key.clone(),
    }
}

pub fn gen_valid_vote(state: &State) -> Vote {
    Vote {
        height:     state.height,
        round:      state.round,
        vote_type:  gen_vote_type(),
        block_hash: gen_valid_hash().as_bytes(),
    }
}

pub fn gen_valid_choke(state: &State, my_pub_key: &Bytes) -> Choke {
    Choke {
        height: state.height,
        round:  state.round,
        from:   UpdateFrom::PrevoteQC(gen_valid_qc(state, my_pub_key)),
    }
}

pub fn gen_invalid_from() -> UpdateFrom {
    match gen_range(0, 100) % 3 {
        0 => UpdateFrom::PrevoteQC(gen_invalid_qc()),
        1 => UpdateFrom::PrecommitQC(gen_invalid_qc()),
        2 => UpdateFrom::ChokeQC(gen_invalid_aggregated_choke()),
        _ => panic!("unreachable!"),
    }
}

pub fn gen_valid_signed_choke(
    choke: Choke,
    crypto: &Arc<OverlordCrypto>,
    my_pub_key: &Bytes,
) -> SignedChoke {
    let signature = crypto
        .sign(crypto.hash(Bytes::from(rlp::encode(&choke_to_hash(&choke)))))
        .expect("sign choke failed");
    SignedChoke {
        signature,
        choke,
        address: my_pub_key.clone(),
    }
}

#[derive(Clone, Debug)]
struct HashChoke {
    height: u64,
    round:  u64,
}

impl Encodable for HashChoke {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2).append(&self.height).append(&self.round);
    }
}

fn choke_to_hash(choke: &Choke) -> HashChoke {
    HashChoke {
        height: choke.height,
        round:  choke.round,
    }
}

pub fn gen_invalid_aggregated_choke() -> AggregatedChoke {
    AggregatedChoke {
        height:    gen_range(u64::MIN, u64::MAX),
        round:     gen_range(u64::MIN, u64::MAX),
        signature: gen_invalid_sig(),
        voters:    vec![gen_invalid_address().as_bytes()],
    }
}

fn gen_vote_type() -> VoteType {
    match gen_range(0, 100) % 2 {
        0 => VoteType::Prevote,
        1 => VoteType::Precommit,
        _ => panic!("unreachable!"),
    }
}
