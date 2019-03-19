pub mod secp256k1;

use std::fmt;

use core_types::Hash;

pub struct Keypair {
    pub private_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

pub trait Crypto {
    fn recover_public_key(hash: &Hash, signature: &[u8]) -> Result<Vec<u8>, CryptoError>;
    fn verify_with_signature(hash: &Hash, signature: &[u8]) -> Result<(), CryptoError>;
    fn gen_keypair() -> Keypair;
    fn sign(hash: &Hash, privkey: &[u8]) -> Result<Vec<u8>, CryptoError>;
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
