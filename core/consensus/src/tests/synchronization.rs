use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use bit_vec::BitVec;
use futures::executor::block_on;
use futures::lock::Mutex;
use overlord::types::{AggregatedSignature, AggregatedVote, Node, SignedVote, Vote, VoteType};
use overlord::{extract_voters, Crypto};
use parking_lot::RwLock;

use common_crypto::{
    BlsCommonReference, BlsPrivateKey, BlsPublicKey, HashValue, PrivateKey, PublicKey,
    Secp256k1PrivateKey, Secp256k1PublicKey, Signature, ToPublicKey,
};
use common_merkle::Merkle;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{CommonConsensusAdapter, Synchronization, SynchronizationAdapter};
use protocol::traits::{Context, ExecutorParams, ExecutorResp, ServiceResponse, TrustFeedback};
use protocol::types::{
    Address, Block, BlockHeader, Bytes, Hash, Hex, MerkleRoot, Metadata, Proof, RawTransaction,
    Receipt, ReceiptResponse, SignedTransaction, TransactionRequest, Validator, ValidatorExtend,
};
use protocol::ProtocolResult;

use crate::status::{CurrentConsensusStatus, StatusAgent};
use crate::synchronization::{OverlordSynchronization, RichBlock};
use crate::util::{convert_hex_to_bls_pubkeys, digest_signed_transactions, OverlordCrypto};
use crate::BlockHeaderField::{PreviousBlockHash, ProofHash, Proposer};
use crate::BlockProofField::{BitMap, HashMismatch, HeightMismatch, WeightNotFound};
use crate::{BlockHeaderField, BlockProofField, ConsensusError};

const PUB_KEY_STR: &str = "02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60";

// Test the blocks gap from 1 to 4.
#[test]
fn sync_gap_test() {
    for gap in [1, 2, 3, 4].iter() {
        let key_tool = get_mock_key_tool();

        let max_height = 10 * *gap;

        let list_rich_block = mock_chained_rich_block(max_height, *gap, &key_tool);

        let remote_blocks = gen_remote_block_hashmap(list_rich_block.0.clone());
        let remote_proofs = gen_remote_proof_hashmap(list_rich_block.1.clone());
        let genesis_block = remote_blocks.read().get(&0).unwrap().clone();

        let local_blocks = Arc::new(RwLock::new(HashMap::new()));
        local_blocks
            .write()
            .insert(genesis_block.header.height, genesis_block.clone());

        let local_transactions = Arc::new(RwLock::new(HashMap::new()));
        let remote_transactions = gen_remote_tx_hashmap(list_rich_block.0.clone());

        let adapter = Arc::new(MockCommonConsensusAdapter::new(
            0,
            local_blocks,
            remote_blocks,
            remote_proofs,
            local_transactions,
            remote_transactions,
            Arc::clone(&key_tool.overlord_crypto),
        ));
        let block_hash = Hash::digest(genesis_block.header.encode_fixed().unwrap());
        let status = CurrentConsensusStatus {
            cycles_price:                1,
            cycles_limit:                300_000_000,
            latest_committed_height:     genesis_block.header.height,
            exec_height:                 genesis_block.header.exec_height,
            current_hash:                block_hash,
            list_confirm_root:           vec![],
            latest_committed_state_root: genesis_block.header.state_root.clone(),
            list_state_root:             vec![],
            list_receipt_root:           vec![],
            list_cycles_used:            vec![],
            current_proof:               genesis_block.header.proof,
            validators:                  genesis_block.header.validators,
            consensus_interval:          3000,
            propose_ratio:               15,
            prevote_ratio:               10,
            precommit_ratio:             10,
            brake_ratio:                 3,
            tx_num_limit:                20000,
            max_tx_size:                 1_073_741_824,
        };
        let status_agent = StatusAgent::new(status);
        let lock = Arc::new(Mutex::new(()));
        let sync = OverlordSynchronization::<_>::new(
            5000,
            Arc::clone(&adapter),
            status_agent.clone(),
            Arc::new(mock_crypto()),
            lock,
        );

        // simulate to get a block
        block_on(sync.receive_remote_block(Context::new(), max_height / 2)).unwrap();

        // get the current consensus status to check if the test works fine
        let status = status_agent.to_inner();
        let block =
            block_on(adapter.get_block_by_height(Context::new(), status.latest_committed_height))
                .unwrap();
        assert_sync(status, block);

        block_on(sync.receive_remote_block(Context::new(), max_height)).unwrap();
        let status = status_agent.to_inner();
        let block =
            block_on(adapter.get_block_by_height(Context::new(), status.latest_committed_height))
                .unwrap();
        assert_sync(status, block);

        let status = status_agent.to_inner();
        // status.current_height is consensus-ed height
        assert_eq!(status.latest_committed_height, max_height);
    }
}

pub type SafeHashMap<K, V> = Arc<RwLock<HashMap<K, V>>>;

pub struct MockCommonConsensusAdapter {
    latest_height:       RwLock<u64>,
    local_blocks:        SafeHashMap<u64, Block>,
    remote_blocks:       SafeHashMap<u64, Block>,
    remote_proofs:       SafeHashMap<u64, Proof>,
    local_transactions:  SafeHashMap<Hash, SignedTransaction>,
    remote_transactions: SafeHashMap<Hash, SignedTransaction>,
    crypto:              Arc<OverlordCrypto>,
}

impl MockCommonConsensusAdapter {
    pub fn new(
        latest_height: u64,
        local_blocks: SafeHashMap<u64, Block>,
        remote_blocks: SafeHashMap<u64, Block>,
        remote_proofs: SafeHashMap<u64, Proof>,
        local_transactions: SafeHashMap<Hash, SignedTransaction>,
        remote_transactions: SafeHashMap<Hash, SignedTransaction>,
        crypto: Arc<OverlordCrypto>,
    ) -> Self {
        Self {
            latest_height: RwLock::new(latest_height),
            local_blocks,
            remote_blocks,
            remote_proofs,
            local_transactions,
            remote_transactions,
            crypto,
        }
    }
}

#[async_trait]
impl SynchronizationAdapter for MockCommonConsensusAdapter {
    fn update_status(
        &self,
        _: Context,
        _: u64,
        _: u64,
        _: u64,
        _: u64,
        _: u64,
        _: u64,
        _: Vec<Validator>,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    fn sync_exec(
        &self,
        _: Context,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp> {
        Ok(exec_txs(params.height, txs).0)
    }

    /// Pull some blocks from other nodes from `begin` to `end`.
    async fn get_block_from_remote(&self, _: Context, height: u64) -> ProtocolResult<Block> {
        Ok(self.remote_blocks.read().get(&height).unwrap().clone())
    }

    /// Pull signed transactions corresponding to the given hashes from other
    /// nodes.
    async fn get_txs_from_remote(
        &self,
        _: Context,
        _: u64,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let map = self.remote_transactions.read();
        let mut txs = vec![];

        for hash in tx_hashes.iter() {
            let tx = map.get(hash).unwrap();
            txs.push(tx.clone())
        }

        Ok(txs)
    }

    async fn get_proof_from_remote(&self, _: Context, height: u64) -> ProtocolResult<Proof> {
        Ok(self.remote_proofs.read().get(&height).unwrap().clone())
    }
}

#[async_trait]
impl CommonConsensusAdapter for MockCommonConsensusAdapter {
    /// Save a block to the database.
    async fn save_block(&self, _: Context, block: Block) -> ProtocolResult<()> {
        self.local_blocks.write().insert(block.header.height, block);
        let mut height = self.latest_height.write();
        *height += 1;
        Ok(())
    }

    async fn save_proof(&self, _: Context, _: Proof) -> ProtocolResult<()> {
        Ok(())
    }

    /// Save some signed transactions to the database.
    async fn save_signed_txs(
        &self,
        _: Context,
        _block_height: u64,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let mut map = self.local_transactions.write();
        for tx in signed_txs.into_iter() {
            map.insert(tx.tx_hash.clone(), tx);
        }
        Ok(())
    }

    async fn save_receipts(&self, _: Context, _: u64, _: Vec<Receipt>) -> ProtocolResult<()> {
        Ok(())
    }

    /// Flush the given transactions in the mempool.
    async fn flush_mempool(&self, _: Context, _: &[Hash]) -> ProtocolResult<()> {
        Ok(())
    }

    /// Get a block corresponding to the given height.
    async fn get_block_by_height(&self, _: Context, height: u64) -> ProtocolResult<Block> {
        Ok(self.local_blocks.read().get(&height).unwrap().clone())
    }

    async fn get_block_header_by_height(
        &self,
        _ctx: Context,
        height: u64,
    ) -> ProtocolResult<BlockHeader> {
        Ok(self
            .local_blocks
            .read()
            .get(&height)
            .unwrap()
            .header
            .clone())
    }

    /// Get the current height from storage.
    async fn get_current_height(&self, _: Context) -> ProtocolResult<u64> {
        Ok(*self.latest_height.read())
    }

    async fn get_txs_from_storage(
        &self,
        _: Context,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let map = self.local_transactions.read();
        let mut txs = vec![];

        for hash in tx_hashes.iter() {
            let tx = map.get(hash).unwrap();
            txs.push(tx.clone())
        }

        Ok(txs)
    }

    async fn broadcast_height(&self, _: Context, _: u64) -> ProtocolResult<()> {
        Ok(())
    }

    fn get_metadata(
        &self,
        _context: Context,
        _state_root: MerkleRoot,
        _height: u64,
        _timestamp: u64,
        _proposer: Address,
    ) -> ProtocolResult<Metadata> {
        Ok(Metadata {
            chain_id:           Hash::from_empty(),
            bech32_address_hrp: "muta".to_owned(),
            common_ref:         Hex::from_string("0x6c747758636859487038".to_string()).unwrap(),
            timeout_gap:        20,
            cycles_limit:       9999,
            cycles_price:       1,
            interval:           3000,
            verifier_list:      mock_verifier_list(),
            propose_ratio:      10,
            prevote_ratio:      10,
            precommit_ratio:    10,
            brake_ratio:        10,
            tx_num_limit:       20000,
            max_tx_size:        1_073_741_824,
        })
    }

    fn tag_consensus(&self, _: Context, _: Vec<Bytes>) -> ProtocolResult<()> {
        Ok(())
    }

    fn report_bad(&self, _ctx: Context, _feedback: TrustFeedback) {}

    fn set_args(
        &self,
        _context: Context,
        _timeout_gap: u64,
        _cycles_limit: u64,
        _max_tx_size: u64,
    ) {
    }

    /// this function verify all info in header except proof and roots
    async fn verify_block_header(&self, ctx: Context, block: &Block) -> ProtocolResult<()> {
        let previous_block = self
            .get_block_by_height(ctx.clone(), block.header.height - 1)
            .await?;

        let previous_block_hash = Hash::digest(previous_block.header.encode_fixed()?);

        if previous_block_hash != block.header.prev_hash {
            log::error!(
                "[consensus] verify_block_header, previous_block_hash: {:?}, block.header.prev_hash: {:?}",
                previous_block_hash,
                block.header.prev_hash
            );
            return Err(
                ConsensusError::VerifyBlockHeader(block.header.height, PreviousBlockHash).into(),
            );
        }

        // the block 0 and 1 's proof is consensus-ed by community
        if block.header.height > 1u64 && block.header.prev_hash != block.header.proof.block_hash {
            log::error!(
                "[consensus] verify_block_header, verifying_block : {:?}",
                block
            );
            return Err(ConsensusError::VerifyBlockHeader(block.header.height, ProofHash).into());
        }

        // verify proposer and validators
        let previous_metadata = self.get_metadata(
            ctx,
            previous_block.header.state_root.clone(),
            previous_block.header.height,
            previous_block.header.timestamp,
            previous_block.header.proposer,
        )?;

        let authority_map = previous_metadata
            .verifier_list
            .iter()
            .map(|v| {
                let address = v.pub_key.decode();
                let node = Node {
                    address:        v.pub_key.decode(),
                    propose_weight: v.propose_weight,
                    vote_weight:    v.vote_weight,
                };
                (address, node)
            })
            .collect::<HashMap<_, _>>();

        // check proposer
        if block.header.height != 0
            && !previous_metadata
                .verifier_list
                .iter()
                .any(|v| v.address == block.header.proposer)
        {
            log::error!(
                "[consensus] verify_block_header, block.header.proposer: {:?}, authority_map: {:?}",
                block.header.proposer,
                authority_map
            );
            return Err(ConsensusError::VerifyBlockHeader(block.header.height, Proposer).into());
        }

        // check validators
        for validator in block.header.validators.iter() {
            let validator_address = Address::from_pubkey_bytes(validator.pub_key.clone());

            if !authority_map.contains_key(&validator.pub_key) {
                log::error!(
                    "[consensus] verify_block_header, validator.address: {:?}, authority_map: {:?}",
                    validator_address,
                    authority_map
                );
                return Err(ConsensusError::VerifyBlockHeader(
                    block.header.height,
                    BlockHeaderField::Validator,
                )
                .into());
            } else {
                let node = authority_map.get(&validator.pub_key).unwrap();

                if node.vote_weight != validator.vote_weight
                    || node.propose_weight != validator.vote_weight
                {
                    log::error!(
                        "[consensus] verify_block_header, validator.address: {:?}, authority_map: {:?}",
                        validator_address,
                        authority_map
                    );
                    return Err(ConsensusError::VerifyBlockHeader(
                        block.header.height,
                        BlockHeaderField::Weight,
                    )
                    .into());
                }
            }
        }

        Ok(())
    }

    async fn verify_proof(
        &self,
        ctx: Context,
        block_header: &BlockHeader,
        proof: &Proof,
    ) -> ProtocolResult<()> {
        // the block 0 has no proof, which is consensus-ed by community, not by chain

        if block_header.height == 0 {
            return Ok(());
        };

        if block_header.height != proof.height {
            log::error!(
                "[consensus] verify_proof, block_header.height: {}, proof.height: {}",
                block_header.height,
                proof.height
            );
            return Err(ConsensusError::VerifyProof(
                block_header.height,
                HeightMismatch(block_header.height, proof.height),
            )
            .into());
        }

        let blockhash = Hash::digest(block_header.clone().encode_fixed()?);

        if blockhash != proof.block_hash {
            log::error!(
                "[consensus] verify_proof, blockhash: {:?}, proof.block_hash: {:?}",
                blockhash,
                proof.block_hash
            );
            return Err(ConsensusError::VerifyProof(block_header.height, HashMismatch).into());
        }

        let previous_block = self
            .get_block_by_height(ctx.clone(), block_header.height - 1)
            .await?;
        // the auth_list for the target should comes from previous height
        let metadata = self.get_metadata(
            ctx.clone(),
            previous_block.header.state_root.clone(),
            previous_block.header.height,
            previous_block.header.timestamp,
            previous_block.header.proposer,
        )?;

        let mut authority_list = metadata
            .verifier_list
            .iter()
            .map(|v| Node {
                address:        v.pub_key.decode(),
                propose_weight: v.propose_weight,
                vote_weight:    v.vote_weight,
            })
            .collect::<Vec<Node>>();

        let signed_voters = extract_voters(&mut authority_list, &proof.bitmap).map_err(|_| {
            log::error!("[consensus] extract_voters fails, bitmap error");
            ConsensusError::VerifyProof(block_header.height, BitMap)
        })?;

        let vote = Vote {
            height:     proof.height,
            round:      proof.round,
            vote_type:  VoteType::Precommit,
            block_hash: proof.block_hash.as_bytes(),
        };

        let vote_hash = self.crypto.hash(protocol::Bytes::from(rlp::encode(&vote)));
        let hex_pubkeys = metadata
            .verifier_list
            .iter()
            .filter_map(|v| {
                if signed_voters.contains(&v.pub_key.decode()) {
                    Some(v.bls_pub_key.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        self.verify_proof_signature(
            ctx.clone(),
            block_header.height,
            vote_hash.clone(),
            proof.signature.clone(),
            hex_pubkeys,
        ).map_err(|e| {
            log::error!("[consensus] verify_proof_signature error, height {}, vote: {:?}, vote_hash:{:?}, sig:{:?}, signed_voter:{:?}",
            block_header.height,
            vote,
            vote_hash,
            proof.signature,
            signed_voters,
            );
            e
        })?;

        let weight_map = authority_list
            .iter()
            .map(|node| (node.address.clone(), node.vote_weight))
            .collect::<HashMap<_, _>>();

        self.verify_proof_weight(ctx.clone(), block_header.height, weight_map, signed_voters)?;

        Ok(())
    }

    fn verify_proof_signature(
        &self,
        _ctx: Context,
        block_height: u64,
        vote_hash: Bytes,
        aggregated_signature_bytes: Bytes,
        vote_keys: Vec<Hex>,
    ) -> ProtocolResult<()> {
        let mut pub_keys = Vec::new();
        for hex in vote_keys.into_iter() {
            pub_keys.push(convert_hex_to_bls_pubkeys(hex)?)
        }

        self.crypto
            .inner_verify_aggregated_signature(vote_hash, pub_keys, aggregated_signature_bytes)
            .map_err(|e| {
                log::error!("[consensus] verify_proof_signature error: {}", e);
                ConsensusError::VerifyProof(block_height, BlockProofField::Signature).into()
            })
    }

    fn verify_proof_weight(
        &self,
        _ctx: Context,
        block_height: u64,
        weight_map: HashMap<Bytes, u32>,
        signed_voters: Vec<Bytes>,
    ) -> ProtocolResult<()> {
        let total_validator_weight: u64 = weight_map.iter().map(|pair| u64::from(*pair.1)).sum();

        let mut accumulator = 0u64;
        for signed_voter_address in signed_voters.iter() {
            if weight_map.contains_key(signed_voter_address) {
                let weight = weight_map.get(signed_voter_address).ok_or_else(|| {
                    log::error!(
                        "[consensus] verify_proof_weight, signed_voter_address: {:?}",
                        hex::encode(signed_voter_address)
                    );
                    ConsensusError::VerifyProof(block_height, WeightNotFound)
                })?;
                accumulator += u64::from(*(weight));
            } else {
                log::error!(
                    "[consensus] verify_proof_weight,signed_voter_address: {:?}",
                    hex::encode(signed_voter_address)
                );

                return Err(
                    ConsensusError::VerifyProof(block_height, BlockProofField::Validator).into(),
                );
            }
        }

        if 3 * accumulator <= 2 * total_validator_weight {
            log::error!(
                "[consensus] verify_proof_weight, accumulator: {}, total: {}",
                accumulator,
                total_validator_weight
            );

            return Err(ConsensusError::VerifyProof(block_height, BlockProofField::Weight).into());
        }
        Ok(())
    }
}

fn mock_crypto() -> OverlordCrypto {
    let priv_key = BlsPrivateKey::try_from(hex::decode("00000000000000000000000000000000d654c7a6747fc2e34808c1ebb1510bfb19b443d639f2fab6dc41fce9f634de37").unwrap().as_ref()).unwrap();
    OverlordCrypto::new(priv_key, HashMap::new(), "muta".into())
}

fn gen_remote_tx_hashmap(list: Vec<RichBlock>) -> SafeHashMap<Hash, SignedTransaction> {
    let mut remote_txs = HashMap::new();

    for rich_block in list.into_iter() {
        for tx in rich_block.txs {
            remote_txs.insert(tx.tx_hash.clone(), tx);
        }
    }

    Arc::new(RwLock::new(remote_txs))
}

fn gen_remote_block_hashmap(list: Vec<RichBlock>) -> SafeHashMap<u64, Block> {
    let mut remote_blocks = HashMap::new();
    for rich_block in list.into_iter() {
        remote_blocks.insert(rich_block.block.header.height, rich_block.block.clone());
    }

    Arc::new(RwLock::new(remote_blocks))
}

fn gen_remote_proof_hashmap(list: Vec<Proof>) -> SafeHashMap<u64, Proof> {
    let mut remote_proof = HashMap::new();
    for proof in list.into_iter() {
        remote_proof.insert(proof.height, proof.clone());
    }

    Arc::new(RwLock::new(remote_proof))
}

fn mock_chained_rich_block(len: u64, gap: u64, key_tool: &KeyTool) -> (Vec<RichBlock>, Vec<Proof>) {
    let mut list_rich_block = vec![];
    let mut list_proof = vec![];

    let genesis_rich_block = mock_genesis_rich_block();
    list_rich_block.push(genesis_rich_block.clone());
    // the proof of block 0 is n/a, we just stuff something here
    list_proof.push(genesis_rich_block.clone().block.header.proof);
    let mut last_rich_block = genesis_rich_block;

    let mut current_height = 1;

    let mut temp_rich_block: Vec<RichBlock> = vec![];

    let mut last_proof: Proof = Proof {
        height:     0,
        round:      0,
        block_hash: Hash::from_hex(
            "0x1122334455667788990011223344556677889900112233445566778899001122",
        )
        .unwrap(),
        signature:  Default::default(),
        bitmap:     Default::default(),
    };

    loop {
        let last_block_hash = Hash::digest(last_rich_block.block.header.encode_fixed().unwrap());
        let last_header = &last_rich_block.block.header;

        let txs = mock_tx_list(3, current_height);
        let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        let order_root = Merkle::from_hashes(tx_hashes.clone())
            .get_root_hash()
            .unwrap();
        let order_signed_transactions_hash = digest_signed_transactions(&txs).unwrap();

        let mut header = BlockHeader {
            chain_id: last_header.chain_id.clone(),
            height: current_height,
            exec_height: current_height,
            prev_hash: last_block_hash.clone(),
            timestamp: 0,
            order_root,
            order_signed_transactions_hash,
            confirm_root: vec![],
            state_root: Hash::from_empty(),
            receipt_root: vec![],
            cycles_used: vec![],
            proposer: Address::from_str("muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705").unwrap(),
            proof: last_proof,
            validator_version: 0,
            validators: vec![Validator {
                pub_key:        Hex::from_string(
                    "0x02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60"
                        .to_owned(),
                )
                .unwrap()
                .decode(),
                propose_weight: 5,
                vote_weight:    5,
            }],
        };

        if last_header.height != 0 && current_height % gap == 0 {
            temp_rich_block.iter().for_each(|rich_block| {
                let height = rich_block.block.header.height;
                let confirm_root = rich_block.block.header.order_root.clone();
                let (exec_resp, receipt_root) = exec_txs(height, &rich_block.txs);

                header.exec_height = height;
                header.confirm_root.push(confirm_root);
                header.state_root = exec_resp.state_root;
                header.receipt_root.push(receipt_root);
                header.cycles_used.push(exec_resp.all_cycles_used);
            });

            temp_rich_block.clear();
        } else if last_header.height != 0 && header.height != 1 {
            header.exec_height -= temp_rich_block.len() as u64 + 1;
        } else if header.height == 1 {
            header.exec_height -= 1;
        }

        let block = Block {
            header,
            ordered_tx_hashes: tx_hashes,
        };

        let rich_block = RichBlock { block, txs };

        list_rich_block.push(rich_block.clone());
        temp_rich_block.push(rich_block.clone());
        last_rich_block = rich_block.clone();

        let current_block_hash = Hash::digest(rich_block.block.header.encode_fixed().unwrap());

        // generate proof for current height and for next block use
        last_proof = mock_proof(current_block_hash.clone(), current_height, 0, &key_tool);

        list_proof.push(last_proof.clone());

        current_height += 1;

        if current_height > len {
            break;
        }
    }

    (list_rich_block, list_proof)
}

fn mock_genesis_rich_block() -> RichBlock {
    let header = BlockHeader {
        chain_id:                       Hash::from_empty(),
        height:                         0,
        exec_height:                    0,
        prev_hash:                      Hash::from_empty(),
        timestamp:                      0,
        order_root:                     Hash::from_empty(),
        order_signed_transactions_hash: Hash::from_empty(),
        confirm_root:                   vec![],
        state_root:                     Hash::from_empty(),
        receipt_root:                   vec![],
        cycles_used:                    vec![],
        proposer:                       "muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705"
            .parse()
            .unwrap(),
        proof:                          Proof {
            height:     0,
            round:      0,
            block_hash: Hash::from_empty(),
            signature:  Bytes::new(),
            bitmap:     Bytes::new(),
        },
        validator_version:              0,
        validators:                     vec![Validator {
            pub_key:        Hex::from_string(
                "0x02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60".to_owned(),
            )
            .unwrap()
            .decode(),
            propose_weight: 0,
            vote_weight:    0,
        }],
    };
    let genesis_block = Block {
        header,
        ordered_tx_hashes: vec![],
    };

    RichBlock {
        block: genesis_block,
        txs:   vec![],
    }
}

fn get_receipt(tx: &SignedTransaction, height: u64) -> Receipt {
    Receipt {
        state_root: MerkleRoot::from_empty(),
        height,
        tx_hash: tx.tx_hash.clone(),
        cycles_used: tx.raw.cycles_limit,
        events: vec![],
        response: ReceiptResponse {
            service_name: "sync".to_owned(),
            method:       "sync_exec".to_owned(),
            response:     ServiceResponse::<String> {
                code:          0,
                succeed_data:  "ok".to_owned(),
                error_message: "".to_owned(),
            },
        },
    }
}

// gen a lot of txs
fn mock_tx_list(num: usize, height: u64) -> Vec<SignedTransaction> {
    let mut txs = vec![];

    for i in 0..num {
        let raw = RawTransaction {
            chain_id:     Hash::from_empty(),
            nonce:        Hash::digest(Bytes::from(format!("{}", i))),
            timeout:      height,
            cycles_price: 1,
            cycles_limit: 1,
            request:      TransactionRequest {
                service_name: "test_service".to_owned(),
                method:       "test_method".to_owned(),
                payload:      "test_payload".to_owned(),
            },
            sender:       Address::from_pubkey_bytes(Bytes::from(
                hex::decode(PUB_KEY_STR).unwrap(),
            ))
            .unwrap(),
        };

        let bytes = raw.encode_fixed().unwrap();

        // sign it vividly
        let hex_privkey =
            hex::decode("5ec982173d54d830b6789cbbbe43eaa2853a5ff752d1ebc1b266cf9790314f8a")
                .unwrap();
        let test_privkey = Secp256k1PrivateKey::try_from(hex_privkey.as_ref()).unwrap();
        let test_pubkey = test_privkey.pub_key();
        let _test_address = Address::from_pubkey_bytes(test_pubkey.to_bytes()).unwrap();

        let tx_hash = Hash::digest(bytes);
        let hash_value = HashValue::try_from(tx_hash.as_bytes().as_ref())
            .ok()
            .unwrap();
        let signature = test_privkey.sign_message(&hash_value);

        let signed_tx = SignedTransaction {
            raw,
            tx_hash,
            pubkey: test_pubkey.to_bytes(),
            signature: signature.to_bytes(),
        };

        txs.push(signed_tx)
    }

    txs
}

// only the bls_private_key in KeyTool.overlordCrypto.private_key signs the
// Vote!!!!!!!
fn mock_proof(block_hash: Hash, height: u64, round: u64, key_tool: &KeyTool) -> Proof {
    let vote = Vote {
        height,
        round,
        vote_type: VoteType::Precommit,
        block_hash: block_hash.as_bytes(),
    };

    let vote_hash = key_tool
        .overlord_crypto
        .hash(Bytes::from(rlp::encode(&vote)));
    let bls_signature = key_tool.overlord_crypto.sign(vote_hash).unwrap();
    let signed_vote = SignedVote {
        voter:     key_tool.signer_node.secp_public_key.to_bytes(),
        signature: bls_signature,
        vote:      vote.clone(),
    };

    let signed_voter = vec![key_tool.signer_node.secp_public_key.to_bytes()]
        .iter()
        .cloned()
        .collect::<HashSet<Bytes>>(); //
    let mut bit_map = BitVec::from_elem(3, false);

    let mut authority_list: Vec<Node> = key_tool
        .verifier_list
        .clone()
        .iter()
        .map(|v| Node {
            address:        v.pub_key.decode(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect::<Vec<_>>();
    authority_list.sort();

    for (index, node) in authority_list.iter().enumerate() {
        if signed_voter.contains(&node.address) {
            bit_map.set(index, true);
        }
    }

    let aggregated_signature = AggregatedSignature {
        signature:      key_tool
            .overlord_crypto
            .aggregate_signatures(vec![signed_vote.signature], vec![signed_vote.voter])
            .unwrap(),
        address_bitmap: Bytes::from(bit_map.to_bytes()),
    };

    let aggregated_vote = AggregatedVote {
        signature: aggregated_signature,

        vote_type: vote.vote_type,
        height,
        round,
        block_hash: block_hash.as_bytes(),
        leader: key_tool.signer_node.secp_public_key.to_bytes(),
    };

    Proof {
        height:     aggregated_vote.height,
        round:      0,
        block_hash: Hash::from_bytes(aggregated_vote.block_hash).unwrap(),
        signature:  aggregated_vote.signature.signature.clone(),
        bitmap:     aggregated_vote.signature.address_bitmap,
    }
}

fn exec_txs(height: u64, txs: &[SignedTransaction]) -> (ExecutorResp, MerkleRoot) {
    let mut receipts = vec![];
    let mut all_cycles_used = 0;

    for tx in txs.iter() {
        let receipt = get_receipt(tx, height);
        all_cycles_used += receipt.cycles_used;
        receipts.push(receipt);
    }
    let receipt_root = Merkle::from_hashes(
        receipts
            .iter()
            .map(|r| Hash::digest(r.to_owned().encode_fixed().unwrap()))
            .collect::<Vec<_>>(),
    )
    .get_root_hash()
    .unwrap_or_else(Hash::from_empty);

    (
        ExecutorResp {
            receipts,
            all_cycles_used,
            state_root: MerkleRoot::from_empty(),
        },
        receipt_root,
    )
}

#[derive(Clone)]
struct SignerNode {
    secp_private_key: Secp256k1PrivateKey,
    secp_public_key:  Secp256k1PublicKey,
}

impl SignerNode {
    pub fn new(secp_private_key: Secp256k1PrivateKey, secp_public_key: Secp256k1PublicKey) -> Self {
        SignerNode {
            secp_private_key,
            secp_public_key,
        }
    }
}

struct KeyTool {
    signer_node:     SignerNode,
    overlord_crypto: Arc<OverlordCrypto>,
    verifier_list:   Vec<ValidatorExtend>,
}

impl KeyTool {
    pub fn new(
        signer_node: SignerNode,
        overlord_crypto: Arc<OverlordCrypto>,
        verifier_list: Vec<ValidatorExtend>,
    ) -> Self {
        KeyTool {
            signer_node,
            overlord_crypto,
            verifier_list,
        }
    }
}

fn get_mock_key_tool() -> KeyTool {
    let hex_privkey =
        hex::decode("5ec982173d54d830b6789cbbbe43eaa2853a5ff752d1ebc1b266cf9790314f8a").unwrap();
    let secp_privkey = Secp256k1PrivateKey::try_from(hex_privkey.as_ref()).unwrap();
    let secp_pubkey: Secp256k1PublicKey = secp_privkey.pub_key();
    let signer_node = SignerNode::new(secp_privkey, secp_pubkey);

    // generate BLS/OverlordCrypto
    let mut bls_priv_key = Vec::new();
    bls_priv_key.extend_from_slice(&[0u8; 16]);
    let mut tmp =
        hex::decode("5ec982173d54d830b6789cbbbe43eaa2853a5ff752d1ebc1b266cf9790314f8a").unwrap();
    bls_priv_key.append(&mut tmp);
    let bls_priv_key = BlsPrivateKey::try_from(bls_priv_key.as_ref()).unwrap();

    let (bls_pub_keys, common_ref) = get_mock_public_keys_and_common_ref();

    let mock_crypto = OverlordCrypto::new(bls_priv_key, bls_pub_keys, common_ref);

    KeyTool::new(signer_node, Arc::new(mock_crypto), mock_verifier_list())
}

fn get_mock_public_keys_and_common_ref() -> (HashMap<Bytes, BlsPublicKey>, BlsCommonReference) {
    let mut bls_pub_keys: HashMap<Bytes, BlsPublicKey> = HashMap::new();

    // weight = 5
    let bls_hex = Hex::from_string("0x04102947214862a503c73904deb5818298a186d68c7907bb609583192a7de6331493835e5b8281f4d9ee705537c0e765580e06f86ddce5867812fceb42eecefd209f0eddd0389d6b7b0100f00fb119ef9ab23826c6ea09aadcc76fa6cea6a32724".to_string()
    ).unwrap();
    let bls_hex = hex::decode(bls_hex.as_string_trim0x()).unwrap();
    bls_pub_keys.insert(
        Hex::from_string(
            "0x02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60".to_owned(),
        )
        .unwrap()
        .decode(),
        BlsPublicKey::try_from(bls_hex.as_ref()).unwrap(),
    );

    // weight = 1
    let bls_hex = Hex::from_string("0x0418e16bd67ce0b58a575f506967706be733c96feef19a06bb37d510000d89905f2f61b7da4d831cb1bb01e2f99833362602a0a252dfd1e95c75c1eadb0db220e3722c9a077b730e7f6cec5f4a55bfc9a4d88db3e6c27684aa8335456824070501".to_string()
    ).unwrap();
    let bls_hex = hex::decode(bls_hex.as_string_trim0x()).unwrap();
    bls_pub_keys.insert(
        Hex::from_string(
            "0x03dbd1dbf3835efb4ec34a360ee671ee1d22425425368edfc5b9ffafc812e86200".to_owned(),
        )
        .unwrap()
        .decode(),
        BlsPublicKey::try_from(bls_hex.as_ref()).unwrap(),
    );

    // weight = 1
    let bls_hex = Hex::from_string("0x040944276f414c46330227f2c0c5a998aba3d400ed19cfc2d31d3e7fcc442ce9f91ea86e172dc3c1b6cedc364bd52ba1cf074529e52337cd80ab32a196a3d42ab46eee25120b44fdd2b5c4268bf3b84c72d068ea83d0530a5461dc30b6a63a60e9".to_string()
    ).unwrap();
    let bls_hex = hex::decode(bls_hex.as_string_trim0x()).unwrap();
    bls_pub_keys.insert(
        Hex::from_string(
            "0x03cba4ae147eb24891d78c9527798577419b7db913b4b03ba548c28f40c5841166".to_owned(),
        )
        .unwrap()
        .decode(),
        BlsPublicKey::try_from(bls_hex.as_ref()).unwrap(),
    );

    let hex_common_ref = hex::decode("6c747758636859487038").unwrap();
    let common_ref: BlsCommonReference =
        std::str::from_utf8(hex_common_ref.as_ref()).unwrap().into();

    (bls_pub_keys, common_ref)
}

fn mock_verifier_list() -> Vec<ValidatorExtend> {
    vec![
        ValidatorExtend {
            bls_pub_key: Hex::from_string("0x04102947214862a503c73904deb5818298a186d68c7907bb609583192a7de6331493835e5b8281f4d9ee705537c0e765580e06f86ddce5867812fceb42eecefd209f0eddd0389d6b7b0100f00fb119ef9ab23826c6ea09aadcc76fa6cea6a32724".to_owned()).unwrap(),
            pub_key: Hex::from_string("0x02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60".to_owned()).unwrap(),
            address: Address::from_str("muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705").unwrap(),
            propose_weight: 5,
            vote_weight:    5,
        },
        ValidatorExtend {
            bls_pub_key: Hex::from_string("0x0418e16bd67ce0b58a575f506967706be733c96feef19a06bb37d510000d89905f2f61b7da4d831cb1bb01e2f99833362602a0a252dfd1e95c75c1eadb0db220e3722c9a077b730e7f6cec5f4a55bfc9a4d88db3e6c27684aa8335456824070501".to_owned()).unwrap(),
            pub_key: Hex::from_string("0x03dbd1dbf3835efb4ec34a360ee671ee1d22425425368edfc5b9ffafc812e86200".to_owned()).unwrap(),
            address: Address::from_str("muta15a8a9ksxe3hhjpw3l7wz7ry778qg8h9wz8y35p").unwrap(),
            propose_weight: 1,
            vote_weight:    1,
        },
        ValidatorExtend {
            bls_pub_key: Hex::from_string("0x040944276f414c46330227f2c0c5a998aba3d400ed19cfc2d31d3e7fcc442ce9f91ea86e172dc3c1b6cedc364bd52ba1cf074529e52337cd80ab32a196a3d42ab46eee25120b44fdd2b5c4268bf3b84c72d068ea83d0530a5461dc30b6a63a60e9".to_owned()).unwrap(),
            pub_key: Hex::from_string("0x03cba4ae147eb24891d78c9527798577419b7db913b4b03ba548c28f40c5841166".to_owned()).unwrap(),
            address: Address::from_str("muta1h99h6f54vytatam3ckftrmvcdpn4jlmnwm6hl0").unwrap(),
            propose_weight: 1,
            vote_weight:    1,
        },
    ]
}

#[rustfmt::skip]
// {
//   "common_ref": "0x6c747758636859487038",
//   "keypairs": [
//     {
//       "index": 1,
//       "private_key": "0x5ec982173d54d830b6789cbbbe43eaa2853a5ff752d1ebc1b266cf9790314f8a",
//       "public_key": "0x02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60",
//       "address": "muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705",
//       "peer_id": "QmTEJkB5QKWsEq37huryZZfVvqBKb54sHnKn9TQcA6j3n9",
//       "bls_public_key": "0x04102947214862a503c73904deb5818298a186d68c7907bb609583192a7de6331493835e5b8281f4d9ee705537c0e765580e06f86ddce5867812fceb42eecefd209f0eddd0389d6b7b0100f00fb119ef9ab23826c6ea09aadcc76fa6cea6a32724"
//     },
//     {
//       "index": 2,
//       "private_key": "0x8dfbd3c689308d29c058cce163984a2ae8d5fc5191ce6b1e18bd1d7b95a8c632",
//       "public_key": "0x03dbd1dbf3835efb4ec34a360ee671ee1d22425425368edfc5b9ffafc812e86200",
//       "address": "muta15a8a9ksxe3hhjpw3l7wz7ry778qg8h9wz8y35p",
//       "peer_id": "QmaEX2TxiC2YJufqcHRigVpnoxahX3hdR1gsFjD5Yf7K1Z",
//       "bls_public_key": "0x0418e16bd67ce0b58a575f506967706be733c96feef19a06bb37d510000d89905f2f61b7da4d831cb1bb01e2f99833362602a0a252dfd1e95c75c1eadb0db220e3722c9a077b730e7f6cec5f4a55bfc9a4d88db3e6c27684aa8335456824070501"
//     },
//     {
//       "index": 3,
//       "private_key": "0xfc659f0ed09a4ba0d2d1836af7520d1a050a7739d598dc98517bbbe7a2e38124",
//       "public_key": "0x03cba4ae147eb24891d78c9527798577419b7db913b4b03ba548c28f40c5841166",
//       "address": "muta1h99h6f54vytatam3ckftrmvcdpn4jlmnwm6hl0",
//       "peer_id": "QmbRmcYD3j2zMr27C6Ga2Bo5xB9t37NyAt36cSvUGYXE2B",
//       "bls_public_key": "0x040944276f414c46330227f2c0c5a998aba3d400ed19cfc2d31d3e7fcc442ce9f91ea86e172dc3c1b6cedc364bd52ba1cf074529e52337cd80ab32a196a3d42ab46eee25120b44fdd2b5c4268bf3b84c72d068ea83d0530a5461dc30b6a63a60e9"
//     },
//     {
//       "index": 4,
//       "private_key": "0x7c01d6539419cffc78ab0779dabe88fad3f70c20ef47a562ac4ba5b7bd704b8e",
//       "public_key": "0x0245a0c291f56c2c5751db1c0bf1ed986e703d29a0fe023df770fe92c7c2347316",
//       "address": "muta16xukzz73l5r6vulk9q697tave8c5mfu33mwud6",
//       "peer_id": "QmeqYprgrXwxzLP7qAFiiJ3Kfi3F6H9PPH2qPCEHr9cRYW",
//       "bls_public_key": "0x041342e9a35278b298a67006cd98d663053e3f7eb72a08ffe9835074e430b2112a866c1c8d981edcd793cb16d459fc952b0464007d876355eea671e74727588bae69740c6a0b49d8142b7b0821a78acd34b4d8012b9ef69444a476e03d5fea5330"
//     }
//   ]
// }

fn assert_sync(status: CurrentConsensusStatus, latest_block: Block) {
    let exec_gap = latest_block.header.height - latest_block.header.exec_height;

    assert_eq!(status.latest_committed_height, latest_block.header.height);
    assert_eq!(status.exec_height, latest_block.header.height);
    assert_eq!(status.current_proof.height, status.latest_committed_height);
    assert_eq!(status.list_confirm_root.len(), exec_gap as usize);
    assert_eq!(status.list_cycles_used.len(), exec_gap as usize);
    assert_eq!(status.list_receipt_root.len(), exec_gap as usize);
}
