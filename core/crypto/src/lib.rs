pub mod secp256k1;

use std::fmt;

use core_types::Hash;

/// "Transform" ensures that the types associated with "Crypto" can be converted to bytes and converted from bytes.
pub trait CryptoTransform: Sized {
    fn from_bytes(data: &[u8]) -> Result<Self, CryptoError>;

    fn as_bytes(&self) -> &[u8];
}

pub trait Crypto: Send + Sync {
    type PrivateKey: CryptoTransform + Clone + Send;
    type PublicKey: CryptoTransform + Clone + Send;
    type Signature: CryptoTransform + Clone + Send;

    fn get_public_key(&self, privkey: &Self::PrivateKey) -> Result<Self::PublicKey, CryptoError>;

    fn verify_with_signature(
        &self,
        hash: &Hash,
        signature: &Self::Signature,
    ) -> Result<Self::PublicKey, CryptoError>;

    fn gen_keypair(&self) -> (Self::PrivateKey, Self::PublicKey);

    fn sign(&self, hash: &Hash, privkey: &Self::PrivateKey)
        -> Result<Self::Signature, CryptoError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    SignatureInvalid,
    PrivateKeyInvalid,
    PublicKeyInvalid,
}

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
