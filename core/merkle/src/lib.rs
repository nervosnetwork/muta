use static_merkle_tree::Tree;

use core_types::{Hash, Receipt};

pub struct Merkle;

impl Merkle {
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
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut root = Vec::with_capacity(left.len() + right.len());
    root.extend_from_slice(left);
    root.extend_from_slice(right);
    Hash::digest(&root)
}
