pub mod secp256k1;

use std::fmt;

use core_types::Hash;

/// "Transform" ensures that the types associated with "Crypto" can be converted to bytes and converted from bytes.
pub trait CryptoTransform: Sized {
    fn from_bytes(data: &[u8]) -> Result<Self, CryptoError>;

    fn as_bytes(&self) -> &[u8];
}

pub trait Crypto {
    type PrivateKey: CryptoTransform;
    type PublicKey: CryptoTransform;
    type Signature: CryptoTransform;

    fn recover_public_key(
        hash: &Hash,
        signature: &Self::Signature,
    ) -> Result<Self::PublicKey, CryptoError>;

    fn verify_with_signature(hash: &Hash, signature: &Self::Signature) -> Result<(), CryptoError>;

    fn gen_keypair() -> (Self::PrivateKey, Self::PublicKey);

    fn sign(hash: &Hash, privkey: &Self::PrivateKey) -> Result<Self::Signature, CryptoError>;
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
