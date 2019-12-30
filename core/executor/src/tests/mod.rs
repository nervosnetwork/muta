mod account_contract;
mod bank_contract;
mod general_state_adapter;
mod trie;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use protocol::traits::executor::{InvokeContext, RcInvokeContext};
use protocol::types::{Address, AssetID, CarryingAsset, Hash, MerkleRoot};

use crate::adapter::GeneralContractStateAdapter;
use crate::trie::MPTTrie;

type MemTrie = MPTTrie<cita_trie::MemoryDB>;

fn create_empty_memdb() -> Arc<cita_trie::MemoryDB> {
    Arc::new(cita_trie::MemoryDB::new(false))
}

fn create_empty_trie(db: Arc<cita_trie::MemoryDB>) -> MemTrie {
    MemTrie::new(Arc::clone(&db))
}

fn create_trie_from_root(root: MerkleRoot, db: Arc<cita_trie::MemoryDB>) -> MemTrie {
    MemTrie::from(root, Arc::clone(&db)).unwrap()
}

fn create_state_adapter() -> GeneralContractStateAdapter<cita_trie::MemoryDB> {
    let memdb = create_empty_memdb();
    let trie = create_empty_trie(Arc::clone(&memdb));
    GeneralContractStateAdapter::new(trie)
}

fn mock_invoke_context(
    caller: Address,
    carrying_asset: Option<CarryingAsset>,
    cycles_used: u64,
    cycles_limit: u64,
    fee_asset_id: AssetID,
) -> RcInvokeContext {
    let ictx = InvokeContext {
        chain_id: Hash::from_empty(),
        cycles_price: 1,
        epoch_id: 1,
        coinbase: caller.clone(),
        caller,
        fee_asset_id,
        cycles_used,
        cycles_limit,
        carrying_asset,
    };

    Rc::new(RefCell::new(ictx))
}
