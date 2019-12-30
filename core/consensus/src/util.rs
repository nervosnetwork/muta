use std::collections::HashMap;
use std::convert::TryFrom;
use std::error::Error;

use overlord::Crypto;

use crate::ConsensusError;
use common_crypto::{
    BlsCommonReference, BlsPrivateKey, BlsPublicKey, BlsSignature, BlsSignatureVerify, HashValue,
    PrivateKey, Signature,
};
use protocol::types::{Address, Hash, MerkleRoot, SignedTransaction};
use protocol::{Bytes, ProtocolError};

pub struct OverlordCrypto {
    private_key: BlsPrivateKey,
    addr_pubkey: HashMap<Bytes, BlsPublicKey>,
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
        let hash = HashValue::try_from(hash.as_ref()).map_err(|_| {
            ProtocolError::from(ConsensusError::Other(
                "failed to convert hash value".to_string(),
            ))
        })?;
        let pub_key = self.addr_pubkey.get(&voter).ok_or_else(|| {
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

        let mut sigs_pubkeys = Vec::with_capacity(signatures.len());
        for item in signatures.iter().zip(voters.iter()) {
            let pub_key = self.addr_pubkey.get(item.1).ok_or_else(|| {
                ProtocolError::from(ConsensusError::Other("lose public key".to_string()))
            })?;
            let signature = BlsSignature::try_from(item.0.as_ref())
                .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;
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
        let mut pub_keys = Vec::new();
        for addr in voters.iter() {
            let pub_key = self.addr_pubkey.get(addr).ok_or_else(|| {
                ProtocolError::from(ConsensusError::Other("lose public key".to_string()))
            })?;
            pub_keys.push(pub_key);
        }

        let aggregate_key = BlsPublicKey::aggregate(pub_keys);
        let aggregated_signature = BlsSignature::try_from(aggregated_signature.as_ref())
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

impl OverlordCrypto {
    pub fn new(
        private_key: BlsPrivateKey,
        addr_pubkey: HashMap<Bytes, BlsPublicKey>,
        common_ref: BlsCommonReference,
    ) -> Self {
        OverlordCrypto {
            addr_pubkey,
            private_key,
            common_ref,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExecuteInfo {
    pub epoch_id:     u64,
    pub chain_id:     Hash,
    pub signed_txs:   Vec<SignedTransaction>,
    pub order_root:   MerkleRoot,
    pub cycles_price: u64,
    pub coinbase:     Address,
}

#[cfg(test)]
mod test {
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
        for i in 0..2 {
            let sig = BlsPrivateKey::try_from(private_keys[i].as_ref())
                .unwrap()
                .sign_message(&hash);
            let pub_key = BlsPublicKey::try_from(public_keys[i].as_ref()).unwrap();
            sigs_and_pub_keys.push((sig, pub_key));
        }

        let signature = BlsSignature::combine(sigs_and_pub_keys.clone());
        let aggregate_key =
            BlsPublicKey::aggregate(sigs_and_pub_keys.iter().map(|s| &s.1).collect::<Vec<_>>());

        let res = signature.verify(&hash, &aggregate_key, &"muta".into());
        println!("{:?}", res);
        assert!(signature
            .verify(&hash, &aggregate_key, &"muta".into())
            .is_ok());
    }
}
