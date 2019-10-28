use std::error::Error;

use bytes::Bytes;
use overlord::{types::AggregatedSignature, Crypto};

use common_crypto::{
    Crypto as Secp256k1Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey,
    Secp256k1PublicKey, Signature,
};

use protocol::types::{Hash, UserAddress};
use protocol::ProtocolError;

use crate::ConsensusError;

#[derive(Clone, Debug)]
pub struct OverlordCrypto {
    public_key:  Secp256k1PublicKey,
    private_key: Secp256k1PrivateKey,
}

impl Crypto for OverlordCrypto {
    fn hash(&self, msg: Bytes) -> Bytes {
        Hash::digest(msg).as_bytes()
    }

    fn sign(&self, hash: Bytes) -> Result<Bytes, Box<dyn Error + Send>> {
        let signature = Secp256k1::sign_message(&hash, &self.private_key.to_bytes())
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?
            .to_bytes();

        let mut res = self.public_key.to_bytes();
        res.extend_from_slice(&signature);
        Ok(res)
    }

    fn verify_signature(
        &self,
        mut signature: Bytes,
        hash: Bytes,
    ) -> Result<Bytes, Box<dyn Error + Send>> {
        let tmp = signature.split_off(33);
        let pub_key = signature;
        let signature = tmp;

        Secp256k1::verify_signature(&hash, &signature, &pub_key)
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;
        let address = UserAddress::from_pubkey_bytes(pub_key)?;
        Ok(address.as_bytes())
    }

    fn aggregate_signatures(
        &self,
        _signatures: Vec<Bytes>,
        _voters: Vec<Bytes>,
    ) -> Result<Bytes, Box<dyn Error + Send>> {
        Ok(Bytes::new())
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
