use rlp::{Encodable, RlpStream};
use static_merkle_tree::{Proof as MerkleProof, ProofNode as MerkleProofNode, Tree};

use core_types::Hash;

pub struct Merkle<T, M> {
    tree: Tree<T, M>,
}

impl<T, M> Merkle<T, M>
where
    T: Default + Clone + PartialEq,
    M: Fn(&T, &T) -> T,
{
    pub fn from_hashes(hashes: Vec<T>, merge: M) -> Self {
        let tree = Tree::from_hashes(hashes, merge);
        Merkle { tree }
    }

    pub fn get_root_hash(&self) -> Option<&T> {
        self.tree.get_root_hash()
    }

    pub fn get_proof_by_input_index(&self, input_index: usize) -> Option<Proof<T>> {
        let proof = self.tree.get_proof_by_input_index(input_index);
        proof.map(Proof::from)
    }
}

pub fn merge(left: &Hash, right: &Hash) -> Hash {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut root = Vec::with_capacity(left.len() + right.len());
    root.extend_from_slice(left);
    root.extend_from_slice(right);
    Hash::digest(&root)
}

#[derive(Debug, Clone)]
pub struct ProofNode<T> {
    pub is_right: bool,
    pub hash: T,
}

/// Structure encodable to RLP
impl<T> Encodable for ProofNode<T>
where
    T: Default + Clone + Encodable,
{
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.is_right);
        s.append(&self.hash);
    }
}

#[derive(Debug, Clone)]
pub struct Proof<T>(pub Vec<ProofNode<T>>);

/// Structure encodable to RLP
impl<T> Encodable for Proof<T>
where
    T: Default + Clone + Encodable,
{
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append_list(&self.0);
    }
}

impl<T> From<MerkleProofNode<T>> for ProofNode<T>
where
    T: Default + Clone,
{
    fn from(node: MerkleProofNode<T>) -> Self {
        ProofNode {
            is_right: node.is_right,
            hash: node.hash,
        }
    }
}

impl<T> From<MerkleProof<T>> for Proof<T>
where
    T: Default + Clone,
{
    fn from(proof: MerkleProof<T>) -> Self {
        Proof(proof.0.into_iter().map(ProofNode::from).collect())
    }
}
