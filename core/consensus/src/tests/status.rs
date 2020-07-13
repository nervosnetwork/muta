use creep::Context;
use rand::random;

use protocol::fixed_codec::FixedCodec;
use protocol::types::{
    Address, Block, BlockHeader, Bytes, Hash, Hex, MerkleRoot, Metadata, Proof, Validator,
    ValidatorExtend,
};

use crate::status::{CurrentConsensusStatus, ExecutedInfo};

const HEIGHT_TEN: u64 = 10;

#[test]
#[should_panic]
fn test_update_by_executed() {
    let mut status = mock_current_status(2);
    let mut status_clone = status.clone();
    let info = mock_executed_info(9);

    status.update_by_executed(info.clone());
    status_clone.exec_height = 9;
    status_clone.list_cycles_used.push(info.cycles_used);
    status_clone
        .list_confirm_root
        .push(info.confirm_root.clone());
    status_clone.list_state_root.push(info.state_root.clone());
    status_clone.list_receipt_root.push(info.receipt_root);
    assert_eq!(status, status_clone);

    let info = mock_executed_info(9);
    status.update_by_executed(info);
    assert_eq!(status, status_clone);

    let info = mock_executed_info(11);
    status.update_by_executed(info);
}

#[test]
#[should_panic]
fn test_update_by_committed() {
    let mut status = mock_current_status(2);
    let status_clone = status.clone();
    let block = mock_block_from_status(&status);
    let metadata = mock_metadata();
    let block_hash = Hash::digest(block.encode_fixed().unwrap());

    status.update_by_committed(
        metadata.clone(),
        block.clone(),
        block_hash.clone(),
        block.header.proof.clone(),
    );

    assert_eq!(status.latest_committed_height, block.header.height);
    assert_eq!(status.current_hash, block_hash);
    assert_eq!(status.latest_committed_state_root, block.header.state_root);
    check_metadata(&status, &metadata);
    check_vec(&status_clone, &status);

    let mut block = mock_block_from_status(&status);
    block.header.height += 1;
    status.update_by_committed(
        metadata,
        block.clone(),
        Hash::digest(block.encode_fixed().unwrap()),
        block.header.proof,
    );
}

fn check_metadata(status: &CurrentConsensusStatus, metadata: &Metadata) {
    assert_eq!(status.consensus_interval, metadata.interval);
    assert_eq!(status.propose_ratio, metadata.propose_ratio);
    assert_eq!(status.prevote_ratio, metadata.prevote_ratio);
    assert_eq!(status.precommit_ratio, metadata.precommit_ratio);
    assert_eq!(status.brake_ratio, metadata.brake_ratio);
    assert_eq!(status.tx_num_limit, metadata.tx_num_limit);
    assert_eq!(status.max_tx_size, metadata.max_tx_size);
}

fn check_vec(status_before: &CurrentConsensusStatus, status_after: &CurrentConsensusStatus) {
    assert!(status_after.list_cycles_used.len() == 1);
    assert!(status_after.list_confirm_root.len() == 1);
    assert!(status_after.list_receipt_root.len() == 1);
    assert!(status_after.list_state_root.len() == 1);

    assert!(status_before
        .list_cycles_used
        .ends_with(&status_after.list_cycles_used));
    assert!(status_before
        .list_confirm_root
        .ends_with(&status_after.list_confirm_root));
    assert!(status_before
        .list_receipt_root
        .ends_with(&status_after.list_receipt_root));
    assert!(status_before
        .list_state_root
        .ends_with(&status_after.list_state_root));
}

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

fn mock_metadata() -> Metadata {
    Metadata {
        chain_id:        mock_hash(),
        common_ref:      Hex::from_string(
            "0xd654c7a6747fc2e34808c1ebb1510bfb19b443d639f2fab6dc41fce9f634de37".to_string(),
        )
        .unwrap(),
        timeout_gap:     random::<u64>(),
        cycles_limit:    random::<u64>(),
        cycles_price:    random::<u64>(),
        verifier_list:   mock_validators_extend(4),
        interval:        random::<u64>(),
        propose_ratio:   random::<u64>(),
        prevote_ratio:   random::<u64>(),
        precommit_ratio: random::<u64>(),
        brake_ratio:     random::<u64>(),
        tx_num_limit:    random::<u64>(),
        max_tx_size:     random::<u64>(),
    }
}

fn mock_validators_extend(len: usize) -> Vec<ValidatorExtend> {
    (0..len)
        .map(|_| ValidatorExtend {
            bls_pub_key:    Hex::from_string(
                "0xd654c7a6747fc2e34808c1ebb1510bfb19b443d639f2fab6dc41fce9f634de37".to_string(),
            )
            .unwrap(),
            peer_id:        mock_peer_id(),
            address:        mock_address(),
            propose_weight: random::<u32>(),
            vote_weight:    random::<u32>(),
        })
        .collect::<Vec<_>>()
}

fn mock_executed_info(height: u64) -> ExecutedInfo {
    ExecutedInfo {
        ctx:          Context::new(),
        exec_height:  height,
        cycles_used:  random::<u64>(),
        state_root:   mock_hash(),
        receipt_root: mock_hash(),
        confirm_root: mock_hash(),
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

fn mock_validators(len: usize) -> Vec<Validator> {
    (0..len).map(|_| mock_validator()).collect::<Vec<_>>()
}

fn mock_validator() -> Validator {
    Validator {
        peer_id:        mock_peer_id().decode(),
        propose_weight: random::<u32>(),
        vote_weight:    random::<u32>(),
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

fn mock_peer_id() -> Hex {
    Hex::from_string(
        "0x1220c7b1dc28da9eeecc7b825f39d0c1e79f87a5cf8a44d888c9f1f1b1ad6be0c79b".to_owned(),
    )
    .unwrap()
}

fn get_random_bytes(len: usize) -> Bytes {
    let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
    Bytes::from(vec)
}
