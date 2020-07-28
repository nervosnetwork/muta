mod engine;
mod status;
mod synchronization;

use rand::random;

use protocol::types::{Address, Block, BlockHeader, Hash, Hex, MerkleRoot, Proof, Validator};
use protocol::Bytes;

use crate::status::CurrentConsensusStatus;

const HEIGHT_TEN: u64 = 10;

fn mock_block_from_status(status: &CurrentConsensusStatus) -> Block {
    let block_header = BlockHeader {
        chain_id:                       mock_hash(),
        height:                         status.latest_committed_height + 1,
        exec_height:                    status.exec_height + 1,
        prev_hash:                      status.current_hash.clone(),
        timestamp:                      random::<u64>(),
        order_root:                     mock_hash(),
        order_signed_transactions_hash: mock_hash(),
        confirm_root:                   vec![status.list_confirm_root.first().cloned().unwrap()],
        state_root:                     status.list_state_root.first().cloned().unwrap(),
        receipt_root:                   vec![status.list_receipt_root.first().cloned().unwrap()],
        cycles_used:                    vec![*status.list_cycles_used.first().unwrap()],
        proposer:                       mock_address(),
        proof:                          mock_proof(status.latest_committed_height),
        validator_version:              1,
        validators:                     mock_validators(4),
    };

    Block {
        header:            block_header,
        ordered_tx_hashes: vec![],
    }
}

fn mock_current_status(exec_lag: u64) -> CurrentConsensusStatus {
    let state_roots = mock_roots(exec_lag);

    CurrentConsensusStatus {
        cycles_price:                random::<u64>(),
        cycles_limit:                random::<u64>(),
        latest_committed_height:     HEIGHT_TEN,
        exec_height:                 HEIGHT_TEN - exec_lag,
        current_hash:                mock_hash(),
        latest_committed_state_root: state_roots.last().cloned().unwrap_or_else(mock_hash),
        list_confirm_root:           mock_roots(exec_lag),
        list_state_root:             state_roots,
        list_receipt_root:           mock_roots(exec_lag),
        list_cycles_used:            (0..exec_lag).map(|_| random::<u64>()).collect::<Vec<_>>(),
        current_proof:               mock_proof(HEIGHT_TEN + exec_lag),
        validators:                  mock_validators(4),
        consensus_interval:          random::<u64>(),
        propose_ratio:               random::<u64>(),
        prevote_ratio:               random::<u64>(),
        precommit_ratio:             random::<u64>(),
        brake_ratio:                 random::<u64>(),
        tx_num_limit:                random::<u64>(),
        max_tx_size:                 random::<u64>(),
    }
}

fn mock_proof(proof_height: u64) -> Proof {
    Proof {
        height:     proof_height,
        round:      random::<u64>(),
        signature:  get_random_bytes(64),
        bitmap:     get_random_bytes(20),
        block_hash: mock_hash(),
    }
}

fn mock_roots(len: u64) -> Vec<MerkleRoot> {
    (0..len).map(|_| mock_hash()).collect::<Vec<_>>()
}

fn mock_hash() -> Hash {
    Hash::digest(get_random_bytes(10))
}

fn mock_address() -> Address {
    let hash = mock_hash();
    Address::from_hash(hash).unwrap()
}

fn get_random_bytes(len: usize) -> Bytes {
    let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
    Bytes::from(vec)
}

fn mock_pub_key() -> Hex {
    Hex::from_string(
        "0x026c184a9016f6f71a234c86b141621f38b68c78602ab06768db4d83682c616004".to_owned(),
    )
    .unwrap()
}

fn mock_validators(len: usize) -> Vec<Validator> {
    (0..len).map(|_| mock_validator()).collect::<Vec<_>>()
}

fn mock_validator() -> Validator {
    Validator {
        pub_key:        mock_pub_key().decode(),
        propose_weight: random::<u32>(),
        vote_weight:    random::<u32>(),
    }
}
