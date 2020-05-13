#![feature(test)]

use static_merkle_tree::Tree;

use protocol::{types::Hash, Bytes};

#[derive(Debug, Clone)]
pub struct ProofNode {
    pub is_right: bool,
    pub hash:     Hash,
}

pub struct Merkle {
    tree: Tree<Hash>,
}

impl Merkle {
    pub fn from_hashes(hashes: Vec<Hash>) -> Self {
        let tree = Tree::from_hashes(hashes, merge);
        Merkle { tree }
    }

    pub fn get_root_hash(&self) -> Option<Hash> {
        match self.tree.get_root_hash() {
            Some(hash) => Some(hash.clone()),
            None => None,
        }
    }

    pub fn get_proof_by_input_index(&self, input_index: usize) -> Option<Vec<ProofNode>> {
        self.tree
            .get_proof_by_input_index(input_index)
            .map(|proof| {
                proof
                    .0
                    .into_iter()
                    .map(|node| ProofNode {
                        is_right: node.is_right,
                        hash:     node.hash,
                    })
                    .collect()
            })
    }
}

fn merge(left: &Hash, right: &Hash) -> Hash {
    let left = left.as_bytes();
    let right = right.as_bytes();

    let mut root = Vec::with_capacity(left.len() + right.len());
    root.extend_from_slice(&left);
    root.extend_from_slice(&right);
    Hash::digest(Bytes::from(root))
}

#[rustfmt::skip]
/// Bench in Intel(R) Core(TM) i7-4770HQ CPU @2.20GHz (8 x 2200):
/// test benches::bench_merkle_1000_hashes  ... bench:   1,167,080 ns/iter (+/- 108,462)
/// test benches::bench_merkle_2000_hashes  ... bench:   2,338,504 ns/iter (+/- 137,184)
/// test benches::bench_merkle_4000_hashes  ... bench:   4,662,601 ns/iter (+/- 231,500)
/// test benches::bench_merkle_8000_hashes  ... bench:   9,336,278 ns/iter (+/- 900,731)
/// test benches::bench_merkle_16000_hashes ... bench:  18,697,547 ns/iter (+/- 1,103,828)

#[cfg(test)]
mod benches {
    extern crate test;

    use rand::random;
    use test::Bencher;

    use super::*;

    fn mock_hash() -> Hash {
        Hash::digest(Bytes::from(
            (0..10).map(|_| random::<u8>()).collect::<Vec<_>>(),
        ))
    }

    fn rand_hashes(size: usize) -> Vec<Hash> {
        (0..size).map(|_| mock_hash()).collect::<Vec<_>>()
    }

    #[bench]
    fn bench_merkle_1000_hashes(b: &mut Bencher) {
        let case = rand_hashes(1000);

        b.iter(|| {
            let _ = Merkle::from_hashes(case.clone());
        });
    }

    #[bench]
    fn bench_merkle_2000_hashes(b: &mut Bencher) {
        let case = rand_hashes(2000);

        b.iter(|| {
            let _ = Merkle::from_hashes(case.clone());
        });
    }

    #[bench]
    fn bench_merkle_4000_hashes(b: &mut Bencher) {
        let case = rand_hashes(4000);

        b.iter(|| {
            let _ = Merkle::from_hashes(case.clone());
        });
    }

    #[bench]
    fn bench_merkle_8000_hashes(b: &mut Bencher) {
        let case = rand_hashes(8000);

        b.iter(|| {
            let _ = Merkle::from_hashes(case.clone());
        });
    }

    #[bench]
    fn bench_merkle_16000_hashes(b: &mut Bencher) {
        let case = rand_hashes(16000);

        b.iter(|| {
            let _ = Merkle::from_hashes(case.clone());
        });
    }
}
