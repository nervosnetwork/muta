pub mod secp256k1;

use std::error::Error;
use std::fmt;

use core_types::{Address, Hash};

/// "Transform" ensures that the types associated with "Crypto" can be converted
/// to bytes and converted from bytes.
pub trait CryptoTransform: Sized {
    fn from_bytes(data: &[u8]) -> Result<Self, CryptoError>;

    fn as_bytes(&self) -> &[u8];
}

pub type CryptoResult<T> = Result<T, CryptoError>;

pub trait Crypto: Send + Sync {
    type PrivateKey: CryptoTransform + Clone + Send + Sync;
    type PublicKey: CryptoTransform + Clone + Send + Sync;
    type Signature: CryptoTransform + Clone + Send + Sync;

    fn get_public_key(&self, privkey: &Self::PrivateKey) -> CryptoResult<Self::PublicKey>;

    fn verify_with_signature(
        &self,
        hash: &Hash,
        signature: &Self::Signature,
    ) -> CryptoResult<Self::PublicKey>;

    fn gen_keypair(&self) -> (Self::PrivateKey, Self::PublicKey);

    fn sign(&self, hash: &Hash, privkey: &Self::PrivateKey) -> CryptoResult<Self::Signature>;

    fn pubkey_to_address(&self, pubkey: &Self::PublicKey) -> Address {
        let pubkey_hash = Hash::digest(&pubkey.as_bytes()[1..]);
        Address::from_hash(&pubkey_hash)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    SignatureInvalid,
    PrivateKeyInvalid,
    PublicKeyInvalid,
}

impl Error for CryptoError {}
impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            CryptoError::SignatureInvalid => "signature invalid".to_string(),
            CryptoError::PrivateKeyInvalid => "private key invalid".to_string(),
            CryptoError::PublicKeyInvalid => "public key invalid".to_string(),
        };
        write!(f, "{}", printable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secp256k1_pubkey_to_address() {
        let secp = secp256k1::Secp256k1::new();
        let priv_key = secp256k1::PrivateKey::from_bytes(
            &hex::decode("37e59320911cdeacd4a39b6dbca45a37dc7c96ec88687cb5242cd51f48504880")
                .unwrap()[..],
        )
        .unwrap();
        let pub_key = secp.get_public_key(&priv_key).unwrap();

        let address = secp.pubkey_to_address(&pub_key);
        assert_eq!(
            hex::encode(address.as_bytes()),
            String::from("bCc4BB994cB0975cdd20C99584318374B3D1522B").to_lowercase()
        );
    }
}
