#![feature(test)]

pub use ophelia::HashValue;
pub use ophelia::{
    BlsSignatureVerify, Crypto, Error, PrivateKey, PublicKey, Signature, ToBlsPublicKey,
    ToPublicKey, UncompressedPublicKey,
};
pub use ophelia_bls_amcl::{BlsCommonReference, BlsPrivateKey, BlsPublicKey, BlsSignature};
pub use ophelia_secp256k1::{
    Secp256k1, Secp256k1PrivateKey, Secp256k1PublicKey, Secp256k1Signature,
};

#[rustfmt::skip]
/// Bench in Intel(R) Core(TM) i7-4770HQ CPU @ 2.20GHz (8 x 2200)
/// test benches::bench_4_aggregated_sig         ... bench:      20,325 ns/iter (+/- 1,251)
/// test benches::bench_8_aggregated_sig         ... bench:      40,178 ns/iter (+/- 4,191)
/// test benches::bench_16_aggregated_sig        ... bench:      78,256 ns/iter (+/- 5,680)
/// test benches::bench_32_aggregated_sig        ... bench:     156,514 ns/iter (+/- 14,312)
/// test benches::bench_64_aggregated_sig        ... bench:     313,124 ns/iter (+/- 16,774)
/// test benches::bench_4_aggregated_sig_verify  ... bench:   4,451,726 ns/iter (+/- 341,019)
/// test benches::bench_8_aggregated_sig_verify  ... bench:   4,347,873 ns/iter (+/- 247,429)
/// test benches::bench_16_aggregated_sig_verify ... bench:   5,034,893 ns/iter (+/- 1,552,969)
/// test benches::bench_32_aggregated_sig_verify ... bench:   4,439,291 ns/iter (+/- 452,905)
/// test benches::bench_64_aggregated_sig_verify ... bench:   4,404,453 ns/iter (+/- 224,377)

#[cfg(test)]
mod benches {
    extern crate test;

    use std::convert::TryFrom;

    use overlord::types::{Vote, VoteType};
    use rand::distributions::Alphanumeric;
    use rand::{random, Rng, RngCore};
    use test::Bencher;

    use protocol::types::Hash;
    use protocol::{Bytes, BytesMut};

    use super::*;

    fn gen_common_ref() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(10)
            .collect::<String>()
    }

    fn mock_block_hash() -> Hash {
        let temp = (0..10).map(|_| random::<u8>()).collect::<Vec<_>>();
        Hash::digest(Bytes::from(temp))
    }

    fn mock_vote() -> Vote {
        Vote {
            height:     0u64,
            round:      0u64,
            vote_type:  VoteType::Prevote,
            block_hash: mock_block_hash().as_bytes(),
        }
    }

    fn gen_key_pair_sigs(
        size: usize,
        keypairs: &mut Vec<(BlsPrivateKey, BlsPublicKey)>,
        sigs: &mut Vec<BlsSignature>,
        hash: &HashValue,
        common_ref: &BlsCommonReference,
    ) {
        for _i in 0..size {
            let seckey = {
                let mut seed = [0u8; 32];
                rand::rngs::OsRng.fill_bytes(&mut seed);
                Hash::digest(BytesMut::from(seed.as_ref()).freeze()).as_bytes()
            };

            let bls_priv_key =
                BlsPrivateKey::try_from([&[0u8; 16], seckey.as_ref()].concat().as_ref()).unwrap();
            let bls_pub_key = bls_priv_key.pub_key(common_ref);

            let sig = bls_priv_key.sign_message(&hash);
            keypairs.push((bls_priv_key, bls_pub_key));
            sigs.push(sig);
        }
    }

    #[bench]
    fn bench_4_aggregated_sig(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            4,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        

        b.iter(move || {
            let _ = BlsSignature::combine(sigs_pubkeys.clone());
        })
    }

    #[bench]
    fn bench_8_aggregated_sig(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            8,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        

        b.iter(move || {
            let _ = BlsSignature::combine(sigs_pubkeys.clone());
        })
    }

    #[bench]
    fn bench_16_aggregated_sig(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            16,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        

        b.iter(move || {
            let _ = BlsSignature::combine(sigs_pubkeys.clone());
        })
    }

    #[bench]
    fn bench_32_aggregated_sig(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            32,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        

        b.iter(move || {
            let _ = BlsSignature::combine(sigs_pubkeys.clone());
        })
    }

    #[bench]
    fn bench_64_aggregated_sig(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            64,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        

        b.iter(move || {
            let _ = BlsSignature::combine(sigs_pubkeys.clone());
        })
    }

    #[bench]
    fn bench_4_aggregated_sig_verify(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            4,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        let aggragated_sig = BlsSignature::combine(sigs_pubkeys);
        let aggregated_key = BlsPublicKey::aggregate(
            priv_pub_keys
                .iter()
                .map(|key_pair| key_pair.1.clone())
                .collect::<Vec<_>>(),
        );

        b.iter(move || {
            aggragated_sig
                .clone()
                .verify(&vote_msg, &aggregated_key, &common_ref)
                .unwrap();
        })
    }

    #[bench]
    fn bench_8_aggregated_sig_verify(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            8,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        let aggragated_sig = BlsSignature::combine(sigs_pubkeys);
        let aggregated_key = BlsPublicKey::aggregate(
            priv_pub_keys
                .iter()
                .map(|key_pair| key_pair.1.clone())
                .collect::<Vec<_>>(),
        );

        b.iter(move || {
            aggragated_sig
                .clone()
                .verify(&vote_msg, &aggregated_key, &common_ref)
                .unwrap();
        })
    }

    #[bench]
    fn bench_16_aggregated_sig_verify(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            16,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        let aggragated_sig = BlsSignature::combine(sigs_pubkeys);
        let aggregated_key = BlsPublicKey::aggregate(
            priv_pub_keys
                .iter()
                .map(|key_pair| key_pair.1.clone())
                .collect::<Vec<_>>(),
        );

        b.iter(move || {
            aggragated_sig
                .clone()
                .verify(&vote_msg, &aggregated_key, &common_ref)
                .unwrap();
        })
    }

    #[bench]
    fn bench_32_aggregated_sig_verify(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            32,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        let aggragated_sig = BlsSignature::combine(sigs_pubkeys);
        let aggregated_key = BlsPublicKey::aggregate(
            priv_pub_keys
                .iter()
                .map(|key_pair| key_pair.1.clone())
                .collect::<Vec<_>>(),
        );

        b.iter(move || {
            aggragated_sig
                .clone()
                .verify(&vote_msg, &aggregated_key, &common_ref)
                .unwrap();
        })
    }

    #[bench]
    fn bench_64_aggregated_sig_verify(b: &mut Bencher) {
        let common_ref: BlsCommonReference = gen_common_ref().as_str().into();
        let vote_msg = HashValue::try_from(
            Hash::digest(Bytes::from(rlp::encode(&mock_vote())))
                .as_bytes()
                .as_ref(),
        )
        .unwrap();

        let mut priv_pub_keys = Vec::new();
        let mut signatures = Vec::new();
        gen_key_pair_sigs(
            64,
            &mut priv_pub_keys,
            &mut signatures,
            &vote_msg,
            &common_ref,
        );

        let sigs_pubkeys = signatures
            .iter()
            .zip(priv_pub_keys.iter())
            .map(|(sig, key_pair)| (sig.clone(), key_pair.1.clone()))
            .collect::<Vec<_>>();
        let aggragated_sig = BlsSignature::combine(sigs_pubkeys);
        let aggregated_key = BlsPublicKey::aggregate(
            priv_pub_keys
                .iter()
                .map(|key_pair| key_pair.1.clone())
                .collect::<Vec<_>>(),
        );

        b.iter(move || {
            aggragated_sig
                .clone()
                .verify(&vote_msg, &aggregated_key, &common_ref)
                .unwrap();
        })
    }
}
