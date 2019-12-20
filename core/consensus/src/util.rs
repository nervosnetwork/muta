use std::error::Error;

use overlord::{types::AggregatedSignature, Crypto};

use common_crypto::{
    Crypto as Secp256k1Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey,
    Secp256k1PublicKey, Signature,
};

use protocol::types::{Hash, UserAddress};
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
