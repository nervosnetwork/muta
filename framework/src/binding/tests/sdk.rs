use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use cita_trie::MemoryDB;

use protocol::traits::{NoopDispatcher, ServiceSDK, Storage};
use protocol::types::{
    Address, Block, BlockHeader, Event, Hash, MerkleRoot, Proof, RawTransaction, Receipt,
    ReceiptResponse, SignedTransaction, TransactionRequest, Validator,
};
use protocol::ProtocolResult;

use crate::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use crate::binding::store::StoreError;
use crate::binding::tests::state::new_state;

#[test]
fn test_service_sdk() {
    let memdb = Arc::new(MemoryDB::new(false));
    let state = new_state(Arc::clone(&memdb), None);
    let rs = Rc::new(RefCell::new(state));

    let arcs = Arc::new(MockStorage {});
    let cq = DefaultChainQuerier::new(Arc::clone(&arcs));

    let mut sdk = DefalutServiceSDK::new(Rc::clone(&rs), Rc::new(cq), NoopDispatcher {});

    // test sdk store bool
    let mut sdk_bool = sdk.alloc_or_recover_bool("test_bool").unwrap();
    sdk_bool.set(true).unwrap();
    assert_eq!(sdk_bool.get().unwrap(), true);

    // test sdk store string
    let mut sdk_string = sdk.alloc_or_recover_string("test_string").unwrap();
    sdk_string.set("hello").unwrap();
    assert_eq!(sdk_string.get().unwrap(), "hello".to_owned());

    // test sdk store uint64
    let mut sdk_uint64 = sdk.alloc_or_recover_uint64("test_uint64").unwrap();
    sdk_uint64.set(99).unwrap();
    assert_eq!(sdk_uint64.get().unwrap(), 99);

    // test sdk map
    let mut sdk_map = sdk.alloc_or_recover_map::<Hash, Bytes>("test_map").unwrap();
    assert_eq!(sdk_map.is_empty().unwrap(), true);

    sdk_map
        .insert(Hash::digest(Bytes::from("key_1")), Bytes::from("val_1"))
        .unwrap();

    assert_eq!(
        sdk_map.get(&Hash::digest(Bytes::from("key_1"))).unwrap(),
        Bytes::from("val_1")
    );

    let mut it = sdk_map.iter();
    assert_eq!(
        it.next().unwrap(),
        (&Hash::digest(Bytes::from("key_1")), Bytes::from("val_1"))
    );
    assert_eq!(it.next().is_none(), true);

    // test sdk array
    let mut sdk_array = sdk.alloc_or_recover_array::<Hash>("test_array").unwrap();
    assert_eq!(sdk_array.is_empty().unwrap(), true);

    sdk_array.push(Hash::digest(Bytes::from("key_1"))).unwrap();

    assert_eq!(
        sdk_array.get(0).unwrap(),
        Hash::digest(Bytes::from("key_1"))
    );

    let mut it = sdk_array.iter();
    assert_eq!(it.next().unwrap(), (0, Hash::digest(Bytes::from("key_1"))));
    assert_eq!(it.next().is_none(), true);

    // test get/set account value
    sdk.set_account_value(&mock_address(), Bytes::from("ak"), Bytes::from("av"))
        .unwrap();
    let account_value: Bytes = sdk
        .get_account_value(&mock_address(), &Bytes::from("ak"))
        .unwrap()
        .unwrap();
    assert_eq!(Bytes::from("av"), account_value);

    // test get/set value
    sdk.set_value(Bytes::from("ak"), Bytes::from("av")).unwrap();
    let value: Bytes = sdk.get_value(&Bytes::from("ak")).unwrap().unwrap();
    assert_eq!(Bytes::from("av"), value);

    // test query chain
    let tx_data = sdk
        .get_transaction_by_hash(&Hash::digest(Bytes::from("param")))
        .unwrap()
        .unwrap();
    assert_eq!(mock_signed_tx(), tx_data);

    let receipt_data = sdk
        .get_receipt_by_hash(&Hash::digest(Bytes::from("param")))
        .unwrap()
        .unwrap();
    assert_eq!(mock_receipt(), receipt_data);

    let block_data = sdk.get_block_by_height(Some(1)).unwrap().unwrap();
    assert_eq!(mock_block(1), block_data);
}

struct MockStorage;

#[async_trait]
impl Storage for MockStorage {
    async fn insert_transactions(&self, _signed_txs: Vec<SignedTransaction>) -> ProtocolResult<()> {
        Ok(())
    }

    async fn insert_block(&self, _block: Block) -> ProtocolResult<()> {
        Ok(())
    }

    async fn insert_receipts(&self, _receipts: Vec<Receipt>) -> ProtocolResult<()> {
        Ok(())
    }

    async fn update_latest_proof(&self, _proof: Proof) -> ProtocolResult<()> {
        Ok(())
    }

    async fn get_transaction_by_hash(&self, _tx_hash: Hash) -> ProtocolResult<SignedTransaction> {
        Ok(mock_signed_tx())
    }

    async fn get_transactions(&self, _hashes: Vec<Hash>) -> ProtocolResult<Vec<SignedTransaction>> {
        Err(StoreError::GetNone.into())
    }

    async fn get_latest_block(&self) -> ProtocolResult<Block> {
        Ok(mock_block(1))
    }

    async fn get_block_by_height(&self, _height: u64) -> ProtocolResult<Block> {
        Ok(mock_block(1))
    }

    async fn get_block_by_hash(&self, _block_hash: Hash) -> ProtocolResult<Block> {
        Err(StoreError::GetNone.into())
    }

    async fn get_receipt(&self, _hash: Hash) -> ProtocolResult<Receipt> {
        Ok(mock_receipt())
    }

    async fn get_receipts(&self, _hash: Vec<Hash>) -> ProtocolResult<Vec<Receipt>> {
        Err(StoreError::GetNone.into())
    }

    async fn get_latest_proof(&self) -> ProtocolResult<Proof> {
        Err(StoreError::GetNone.into())
    }

    async fn update_overlord_wal(&self, _info: Bytes) -> ProtocolResult<()> {
        Ok(())
    }

    async fn load_overlord_wal(&self) -> ProtocolResult<Bytes> {
        Err(StoreError::GetNone.into())
    }
}

// #####################
// Mock Primitive
// #####################

pub fn mock_address() -> Address {
    let hash = mock_hash();
    Address::from_hash(hash).unwrap()
}

pub fn mock_hash() -> Hash {
    Hash::digest(Bytes::from("mock"))
}

pub fn mock_merkle_root() -> MerkleRoot {
    Hash::digest(Bytes::from("mock"))
}

// #####################
// Mock Transaction
// #####################

pub fn mock_transaction_request() -> TransactionRequest {
    TransactionRequest {
        service_name: "mock-service".to_owned(),
        method:       "mock-method".to_owned(),
        payload:      "mock-payload".to_owned(),
    }
}

pub fn mock_raw_tx() -> RawTransaction {
    RawTransaction {
        chain_id:     mock_hash(),
        nonce:        mock_hash(),
        timeout:      100,
        cycles_price: 1,
        cycles_limit: 100,
        request:      mock_transaction_request(),
    }
}

pub fn mock_signed_tx() -> SignedTransaction {
    SignedTransaction {
        raw:       mock_raw_tx(),
        tx_hash:   mock_hash(),
        pubkey:    Default::default(),
        signature: Default::default(),
    }
}

// #####################
// Mock Receipt
// #####################

pub fn mock_receipt() -> Receipt {
    Receipt {
        state_root:  mock_merkle_root(),
        height:      13,
        tx_hash:     mock_hash(),
        cycles_used: 100,
        events:      vec![mock_event()],
        response:    mock_receipt_response(),
    }
}

pub fn mock_receipt_response() -> ReceiptResponse {
    ReceiptResponse {
        service_name: "mock-service".to_owned(),
        method:       "mock-method".to_owned(),
        ret:          "mock-ret".to_owned(),
        is_error:     false,
    }
}

pub fn mock_event() -> Event {
    Event {
        service: "mock-event".to_owned(),
        data:    "mock-data".to_owned(),
    }
}

// #####################
// Mock Block
// #####################

pub fn mock_validator() -> Validator {
    Validator {
        address:        mock_address(),
        propose_weight: 1,
        vote_weight:    1,
    }
}

pub fn mock_proof() -> Proof {
    Proof {
        height:     4,
        round:      99,
        block_hash: mock_hash(),
        signature:  Default::default(),
        bitmap:     Default::default(),
    }
}

pub fn mock_block_header() -> BlockHeader {
    BlockHeader {
        chain_id:          mock_hash(),
        height:            42,
        exec_height:       41,
        pre_hash:          mock_hash(),
        timestamp:         420_000_000,
        logs_bloom:        Default::default(),
        order_root:        mock_merkle_root(),
        confirm_root:      vec![mock_hash(), mock_hash()],
        state_root:        mock_merkle_root(),
        receipt_root:      vec![mock_hash(), mock_hash()],
        cycles_used:       vec![999_999],
        proposer:          mock_address(),
        proof:             mock_proof(),
        validator_version: 1,
        validators:        vec![
            mock_validator(),
            mock_validator(),
            mock_validator(),
            mock_validator(),
        ],
    }
}

pub fn mock_block(order_size: usize) -> Block {
    Block {
        header:            mock_block_header(),
        ordered_tx_hashes: (0..order_size).map(|_| mock_hash()).collect(),
    }
}
