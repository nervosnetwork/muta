pub mod secp256k1;

use std::fmt;

use core_types::Hash;

/// "Transform" ensures that the types associated with "Crypto" can be converted to bytes and converted from bytes.
pub trait Transform {
    fn from_slice(data: &[u8]) -> Self;

    fn as_bytes(&self) -> &[u8];
}

pub trait Crypto {
    type PrivateKey: Transform;
    type PublicKey: Transform;
    type Signature: Transform;

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
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            CryptoError::SignatureInvalid => "signature invalid".to_string(),
            CryptoError::PrivateKeyInvalid => "private key invalid".to_string(),
        };
        write!(f, "{}", printable)
    }
}
