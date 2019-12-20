use std::error::Error;

use overlord::{types::AggregatedSignature, Crypto};

use common_crypto::{
    Crypto as Secp256k1Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey,
    Secp256k1PublicKey, Signature,
};

use protocol::types::{Address, Hash, MerkleRoot, SignedTransaction, UserAddress};
use protocol::{BytesMut, ProtocolError};

use crate::ConsensusError;

#[derive(Clone, Debug)]
pub struct OverlordCrypto {
    public_key:  Secp256k1PublicKey,
    private_key: Secp256k1PrivateKey,
}

impl Crypto for OverlordCrypto {
    fn hash(&self, msg: bytes::Bytes) -> bytes::Bytes {
        let msg = BytesMut::from(msg.as_ref()).freeze();
        bytes::Bytes::from(Hash::digest(msg).as_bytes().as_ref())
    }

    fn sign(&self, hash: bytes::Bytes) -> Result<bytes::Bytes, Box<dyn Error + Send>> {
        let hash = BytesMut::from(hash.as_ref()).freeze();
        let signature = Secp256k1::sign_message(&hash, &self.private_key.to_bytes())
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?
            .to_bytes();

        let mut res = bytes::Bytes::from(self.public_key.to_bytes().as_ref());
        res.extend_from_slice(&signature.as_ref());
        Ok(res)
    }

    fn verify_signature(
        &self,
        mut signature: bytes::Bytes,
        hash: bytes::Bytes,
    ) -> Result<bytes::Bytes, Box<dyn Error + Send>> {
        let tmp = signature.split_off(33);
        let pub_key = signature;
        let signature = tmp;

        let hash = BytesMut::from(hash.as_ref()).freeze();
        let pub_key = BytesMut::from(pub_key.as_ref()).freeze();

        Secp256k1::verify_signature(&hash, &signature, &pub_key)
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;
        let address = UserAddress::from_pubkey_bytes(pub_key)?;
        Ok(bytes::Bytes::from(address.as_bytes().as_ref()))
    }

    fn aggregate_signatures(
        &self,
        _signatures: Vec<bytes::Bytes>,
        _voters: Vec<bytes::Bytes>,
    ) -> Result<bytes::Bytes, Box<dyn Error + Send>> {
        Ok(bytes::Bytes::new())
    }

    fn verify_aggregated_signature(
        &self,
        _aggregated_signature: AggregatedSignature,
    ) -> Result<(), Box<dyn Error + Send>> {
        Ok(())
    }
}

impl OverlordCrypto {
    pub fn new(public_key: Secp256k1PublicKey, private_key: Secp256k1PrivateKey) -> Self {
        OverlordCrypto {
            public_key,
            private_key,
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
    use common_crypto::{
        BlsPrivateKey, BlsPublicKey, BlsSignature, BlsSignatureVerify, HashValue, PrivateKey,
        PublicKey, Signature,
    };
    use std::convert::TryFrom;

    use super::*;

    #[test]
    fn test_bls_amcl() {
        let private_keys = vec![
            hex::decode("000000000000000000000000000000005300a6fe476044d019e41ea1e60a238bee604572399791d594025e139029eda1").unwrap(),
            hex::decode("000000000000000000000000000000004a523df5ba1277c2aab8abcbd36992a185385471e64e1f2903eb5eebb6d0322b").unwrap(),
            hex::decode("0000000000000000000000000000000059fddca68fba89f1bac6f657505e6de473fe64490740c557d2bd6a1ecfe1c297").unwrap(),
            hex::decode("000000000000000000000000000000006cc70c90c27a4057aa6a93c5c05a04de601612d1b477e932ab9aba25c52a456b").unwrap(),
        ];

        let public_keys = vec![
            hex::decode("0411ebec3718c83799b7dfc357463e191805d8dc4d1062ae1eeb9a1a6263e75023507a640e13e6ca91ce5a6c5dbf485fa708d56495a9d228824673dd51b2e699fb929a596b4ea73f9bf8b7c451768a0161f3bfe6573b2b127b754a903d74a3dd27").unwrap(),
            hex::decode("0414ddfa41c71ce851fd2d999d104ddd1c27b41e7de7c463b8a39ea16e2b4ca5288e7584782ea61c5f07b66ef983aea4aa1637fa8b0e146d85272fed875d9ac0d717caaca8566e3e61c06184f342f6f1b253dfedf8a71bbc1297b9b471abba4963").unwrap(),
            hex::decode("04045f930543a0b1adedf358c6cd70be3ac6deaad55c85c46cd466cac9750b3e19826b8cf3e12c06aa6cbe7d410398160e0059e9819f9484e7b1cedb9ff28ea077b62a8917ce4b627acc08839d021e4a286c8bfe1ad98a061c49cfa83be5610c3b").unwrap(),
            hex::decode("040f25cefe64dfeca7e8a3683debe42b5f23ed5c7e6b95d1ef0c206a7407bbc494103df834f23d4d12d789b3926038141e087edfdf84d4ee9244a7ca92e1d4ead41ef4597d3598a3f3fbb3c73c327d4d33fb67f404ef7eeceb6238a66810b8cf5e").unwrap(),
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
        assert!(signature.verify(&hash, &aggregate_key, &"muta".into()).is_ok());
    }
}
