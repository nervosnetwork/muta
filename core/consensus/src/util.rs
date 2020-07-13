use std::collections::HashMap;
use std::convert::TryFrom;
use std::error::Error;

use bytes::buf::BufMut;
use bytes::BytesMut;
use overlord::Crypto;
use parking_lot::RwLock;

use crate::ConsensusError;
use common_crypto::{
    BlsCommonReference, BlsPrivateKey, BlsPublicKey, BlsSignature, BlsSignatureVerify, HashValue,
    PrivateKey, Signature,
};
use protocol::fixed_codec::FixedCodec;
use protocol::traits::Context;
use protocol::types::{Address, Hash, Hex, MerkleRoot, SignedTransaction};
use protocol::{Bytes, ProtocolError, ProtocolResult};

pub struct OverlordCrypto {
    private_key: BlsPrivateKey,
    addr_pubkey: RwLock<HashMap<Bytes, BlsPublicKey>>,
    common_ref:  BlsCommonReference,
}

impl Crypto for OverlordCrypto {
    fn hash(&self, msg: Bytes) -> Bytes {
        Hash::digest(msg).as_bytes()
    }

    fn sign(&self, hash: Bytes) -> Result<Bytes, Box<dyn Error + Send>> {
        let hash = HashValue::try_from(hash.as_ref()).map_err(|_| {
            ProtocolError::from(ConsensusError::Other(
                "failed to convert hash value".to_string(),
            ))
        })?;
        let sig = self.private_key.sign_message(&hash);
        Ok(sig.to_bytes())
    }

    fn verify_signature(
        &self,
        signature: Bytes,
        hash: Bytes,
        voter: Bytes,
    ) -> Result<(), Box<dyn Error + Send>> {
        let map = self.addr_pubkey.read();
        let hash = HashValue::try_from(hash.as_ref()).map_err(|_| {
            ProtocolError::from(ConsensusError::Other(
                "failed to convert hash value".to_string(),
            ))
        })?;
        let pub_key = map.get(&voter).ok_or_else(|| {
            ProtocolError::from(ConsensusError::Other("lose public key".to_string()))
        })?;
        let signature = BlsSignature::try_from(signature.as_ref())
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;

        signature
            .verify(&hash, &pub_key, &self.common_ref)
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;
        Ok(())
    }

    fn aggregate_signatures(
        &self,
        signatures: Vec<Bytes>,
        voters: Vec<Bytes>,
    ) -> Result<Bytes, Box<dyn Error + Send>> {
        if signatures.len() != voters.len() {
            return Err(ProtocolError::from(ConsensusError::Other(
                "signatures length does not match voters length".to_string(),
            ))
            .into());
        }

        let map = self.addr_pubkey.read();
        let mut sigs_pubkeys = Vec::with_capacity(signatures.len());
        for (sig, addr) in signatures.iter().zip(voters.iter()) {
            let signature = BlsSignature::try_from(sig.as_ref())
                .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;

            let pub_key = map.get(addr).ok_or_else(|| {
                ProtocolError::from(ConsensusError::Other("lose public key".to_string()))
            })?;

            sigs_pubkeys.push((signature, pub_key.to_owned()));
        }

        let sig = BlsSignature::combine(sigs_pubkeys);
        Ok(sig.to_bytes())
    }

    fn verify_aggregated_signature(
        &self,
        aggregated_signature: Bytes,
        hash: Bytes,
        voters: Vec<Bytes>,
    ) -> Result<(), Box<dyn Error + Send>> {
        let map = self.addr_pubkey.read();
        let mut pub_keys = Vec::new();
        for addr in voters.iter() {
            let pub_key = map.get(addr).ok_or_else(|| {
                ProtocolError::from(ConsensusError::Other("lose public key".to_string()))
            })?;
            pub_keys.push(pub_key.clone());
        }

        self.inner_verify_aggregated_signature(hash, pub_keys, aggregated_signature)?;
        Ok(())
    }
}

impl OverlordCrypto {
    pub fn new(
        private_key: BlsPrivateKey,
        pubkey_to_bls_pubkey: HashMap<Bytes, BlsPublicKey>,
        common_ref: BlsCommonReference,
    ) -> Self {
        OverlordCrypto {
            addr_pubkey: RwLock::new(pubkey_to_bls_pubkey),
            private_key,
            common_ref,
        }
    }

    pub fn update(&self, new_addr_pubkey: HashMap<Bytes, BlsPublicKey>) {
        let mut map = self.addr_pubkey.write();

        *map = new_addr_pubkey;
    }

    pub fn inner_verify_aggregated_signature(
        &self,
        hash: Bytes,
        pub_keys: Vec<BlsPublicKey>,
        signature: Bytes,
    ) -> ProtocolResult<()> {
        let aggregate_key = BlsPublicKey::aggregate(pub_keys);
        let aggregated_signature = BlsSignature::try_from(signature.as_ref())
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;
        let hash = HashValue::try_from(hash.as_ref()).map_err(|_| {
            ProtocolError::from(ConsensusError::Other(
                "failed to convert hash value".to_string(),
            ))
        })?;

        aggregated_signature
            .verify(&hash, &aggregate_key, &self.common_ref)
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ExecuteInfo {
    pub ctx:          Context,
    pub height:       u64,
    pub chain_id:     Hash,
    pub block_hash:   Hash,
    pub signed_txs:   Vec<SignedTransaction>,
    pub order_root:   MerkleRoot,
    pub cycles_price: u64,
    pub proposer:     Address,
    pub timestamp:    u64,
    pub cycles_limit: u64,
}

pub fn check_list_roots<T: Eq>(cache_roots: &[T], block_roots: &[T]) -> bool {
    block_roots.len() <= cache_roots.len()
        && cache_roots
            .iter()
            .zip(block_roots.iter())
            .all(|(c_root, e_root)| c_root == e_root)
}

pub fn digest_signed_transactions(signed_txs: &[SignedTransaction]) -> ProtocolResult<Hash> {
    if signed_txs.is_empty() {
        return Ok(Hash::from_empty());
    }

    let mut list_bytes = BytesMut::new();

    for signed_tx in signed_txs.iter() {
        let bytes = signed_tx.encode_fixed()?;
        list_bytes.put(bytes);
    }

    Ok(Hash::digest(list_bytes.freeze()))
}

pub fn convert_hex_to_bls_pubkeys(hex: Hex) -> ProtocolResult<BlsPublicKey> {
    let hex_pubkey = hex::decode(hex.as_string_trim0x())
        .map_err(|e| ConsensusError::Other(format!("from hex error {:?}", e)))?;
    let ret = BlsPublicKey::try_from(hex_pubkey.as_ref())
        .map_err(|e| ConsensusError::CryptoErr(Box::new(e)))?;
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bls_amcl() {
        let private_keys = vec![
            hex::decode("000000000000000000000000000000001abd6ffdb44427d9e1fcb6f84e7fe7d98f2b5b205b30a94992ec24d94bb0c970").unwrap(),
            hex::decode("00000000000000000000000000000000320b11d7c1ae66fdad1b4a75221244ae2d84903d3548c581d7d30dc135aac817").unwrap(),
            hex::decode("000000000000000000000000000000006a41e900d0426e615ca9d9393e6792baf9bda4398d5d407e59f77cb6c6f393cc").unwrap(),
            hex::decode("00000000000000000000000000000000125d81e0eb0a9c3746d868bf3b4f07760fdd430daded41d92f53b4e484ef3415").unwrap(),
        ];

        let public_keys = vec![
            hex::decode("041054fe9a65be0891094ed37fb3655e3ffb12353bc0a1b4f8673b52ad65d1ca481780cf7e988eb8dcdc05d8352f03605b0d11afb2525b3f1b55ec694509248bcfead39cbb292725d710e2a509c77ed051d1d49e15e429cf6d12b9be7c02179612").unwrap(),
            hex::decode("040c15c82ed07dc866ab7c3af3a070eb4340ac0439bf12bb49cbed5797d52707e009f7c17414777b0213b9a55c8a5c08290ce40c366d59322db418b7ff41277090bd25614174763c9fd725ede1f65f3e61ca9acdb35f59e33d556e738add14d536").unwrap(),
            hex::decode("040b3118acefdfbb11ded262a7f3c90dfca4fbc0200a92b4f6bb80210ab85e39f79458f7d47f7cb06864df0571e7591a4e0858df0b52a4c3ae19ae3adc32e1da0ec4cbdca108365ee433becdb1ccebb1b339647788dfad94ebae1cbd770fcfa4e5").unwrap(),
            hex::decode("040709f204e3ec5b8bdd9f2bb6edc9cb1704fc1e4952661ba7532ea8e37f3b159b8d41987ee6707d32bdf494e2deb00b7f049a4670a5ce1ad8e429fcacc5bbc69cb03b71a7f1d831d0b47dda5e62642d420ff0a545950cb1db19d42fe04e2c91d2").unwrap(),
        ];

        let msg = Hash::digest(Bytes::from("muta-consensus"));
        let hash = HashValue::try_from(msg.as_bytes().as_ref()).unwrap();
        let mut sigs_and_pub_keys = Vec::new();
        for i in 0..3 {
            let sig = BlsPrivateKey::try_from(private_keys[i].as_ref())
                .unwrap()
                .sign_message(&hash);
            let pub_key = BlsPublicKey::try_from(public_keys[i].as_ref()).unwrap();
            sigs_and_pub_keys.push((sig, pub_key));
        }

        let signature = BlsSignature::combine(sigs_and_pub_keys.clone());
        let aggregate_key = BlsPublicKey::aggregate(
            sigs_and_pub_keys
                .iter()
                .map(|s| s.1.clone())
                .collect::<Vec<_>>(),
        );

        let res = signature.verify(&hash, &aggregate_key, &"muta".into());
        println!("{:?}", res);
        assert!(res.is_ok());
    }

    #[test]
    fn test_aggregate_pubkeys_order() {
        let public_keys = vec![
            hex::decode("041054fe9a65be0891094ed37fb3655e3ffb12353bc0a1b4f8673b52ad65d1ca481780cf7e988eb8dcdc05d8352f03605b0d11afb2525b3f1b55ec694509248bcfead39cbb292725d710e2a509c77ed051d1d49e15e429cf6d12b9be7c02179612").unwrap(),
            hex::decode("040c15c82ed07dc866ab7c3af3a070eb4340ac0439bf12bb49cbed5797d52707e009f7c17414777b0213b9a55c8a5c08290ce40c366d59322db418b7ff41277090bd25614174763c9fd725ede1f65f3e61ca9acdb35f59e33d556e738add14d536").unwrap(),
            hex::decode("040b3118acefdfbb11ded262a7f3c90dfca4fbc0200a92b4f6bb80210ab85e39f79458f7d47f7cb06864df0571e7591a4e0858df0b52a4c3ae19ae3adc32e1da0ec4cbdca108365ee433becdb1ccebb1b339647788dfad94ebae1cbd770fcfa4e5").unwrap(),
            hex::decode("040709f204e3ec5b8bdd9f2bb6edc9cb1704fc1e4952661ba7532ea8e37f3b159b8d41987ee6707d32bdf494e2deb00b7f049a4670a5ce1ad8e429fcacc5bbc69cb03b71a7f1d831d0b47dda5e62642d420ff0a545950cb1db19d42fe04e2c91d2").unwrap(),
        ];
        let mut pub_keys = public_keys
            .into_iter()
            .map(|pk| BlsPublicKey::try_from(pk.as_ref()).unwrap())
            .collect::<Vec<_>>();
        let pk_1 = BlsPublicKey::aggregate(pub_keys.clone());
        pub_keys.reverse();
        let pk_2 = BlsPublicKey::aggregate(pub_keys);
        assert_eq!(pk_1, pk_2);
    }

    #[test]
    fn test_zip_roots() {
        let roots_1 = vec![1, 2, 3, 4, 5];
        let roots_2 = vec![1, 2, 3];
        let roots_3 = vec![];
        let roots_4 = vec![1, 2];
        let roots_5 = vec![3, 4, 5, 6, 8];

        assert!(check_list_roots(&roots_1, &roots_2));
        assert!(!check_list_roots(&roots_3, &roots_2));
        assert!(!check_list_roots(&roots_4, &roots_2));
        assert!(!check_list_roots(&roots_5, &roots_2));
    }

    #[test]
    fn test_convert_from_hex() {
        let hex_str = "0x04188ef9488c19458a963cc57b567adde7db8f8b6bec392d5cb7b67b0abc1ed6cd966edc451f6ac2ef38079460eb965e890d1f576e4039a20467820237cda753f07a8b8febae1ec052190973a1bcf00690ea8fc0168b3fbbccd1c4e402eda5ef22";
        assert!(
            convert_hex_to_bls_pubkeys(Hex::from_string(String::from(hex_str)).unwrap()).is_ok()
        );
    }
}
