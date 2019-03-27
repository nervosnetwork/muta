use static_merkel_tree::Tree;

use core_types::{Hash, Receipt};

pub struct Merkel;

impl Merkel {
    pub fn receipts_root(receipts: &[Receipt]) -> Option<Hash> {
        let hahses: Vec<Hash> = receipts.iter().map(|receipt| receipt.hash()).collect();
        Self::hashes_root(&hahses)
    }

    pub fn hashes_root(hashes: &[Hash]) -> Option<Hash> {
        let tree = Tree::from_hashes(hashes.to_vec(), merge);
        tree.get_root_hash().cloned()
    }
}

fn merge(left: &Hash, right: &Hash) -> Hash {
    let mut root = Vec::with_capacity(left.as_ref().len() + right.as_ref().len());
    root.extend_from_slice(&left.as_ref());
    root.extend_from_slice(&right.as_ref());
    Hash::from_raw(&root)
}
