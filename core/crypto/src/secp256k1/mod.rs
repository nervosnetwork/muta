use secp256k1::{
    constants,
    key::{PublicKey as RawPublicKey, SecretKey},
    rand, All, Message, RecoverableSignature, RecoveryId, Secp256k1 as RawSecp256k1,
};

use core_types::{Address, Hash};

use crate::{Crypto, CryptoError, CryptoTransform};

pub struct Secp256k1 {
    secp: RawSecp256k1<All>,
}

#[derive(Clone)]
pub struct PrivateKey([u8; constants::SECRET_KEY_SIZE]);

impl CryptoTransform for PrivateKey {
    fn from_bytes(data: &[u8]) -> Result<Self, CryptoError> {
        if data.len() != constants::SECRET_KEY_SIZE {
            return Err(CryptoError::PrivateKeyInvalid);
        }

        let mut privkey = [0u8; constants::SECRET_KEY_SIZE];
        privkey[..].copy_from_slice(&data[..]);
        Ok(PrivateKey(privkey))
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(Clone)]
pub struct PublicKey([u8; constants::PUBLIC_KEY_SIZE]);

impl CryptoTransform for PublicKey {
    fn from_bytes(data: &[u8]) -> Result<Self, CryptoError> {
        if data.len() != constants::PUBLIC_KEY_SIZE {
            return Err(CryptoError::PublicKeyInvalid);
        }

        let mut pubkey = [0u8; constants::PUBLIC_KEY_SIZE];
        pubkey[..].copy_from_slice(&data[..]);
        Ok(PublicKey(pubkey))
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(Clone)]
pub struct Signature([u8; constants::COMPACT_SIGNATURE_SIZE + 1]);

impl CryptoTransform for Signature {
    fn from_bytes(data: &[u8]) -> Result<Self, CryptoError> {
        if data.len() != constants::COMPACT_SIGNATURE_SIZE + 1 {
            return Err(CryptoError::SignatureInvalid);
        }

        let mut signatue = [0u8; constants::COMPACT_SIGNATURE_SIZE + 1];
        signatue[..].copy_from_slice(&data[..]);
        Ok(Signature(signatue))
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Crypto for Secp256k1 {
    type PrivateKey = PrivateKey;
    type PublicKey = PublicKey;
    type Signature = Signature;

    fn get_public_key(&self, privkey: &Self::PrivateKey) -> Result<Self::PublicKey, CryptoError> {
        let privkey = SecretKey::from_slice(privkey.as_bytes())
            .map_err(|_| CryptoError::PrivateKeyInvalid)?;
        let pubkey = RawPublicKey::from_secret_key(&self.secp, &privkey);
        Ok(PublicKey(pubkey.serialize()))
    }

    fn verify_with_signature(
        &self,
        hash: &Hash,
        signature: &Self::Signature,
    ) -> Result<Self::PublicKey, CryptoError> {
        let msg = self.msg(hash)?;
        let sig = self.signature(signature.as_bytes())?;
        let pubkey = self.recover(&msg, &sig)?;

        self.verify(&msg, &sig, &pubkey)?;
        Ok(PublicKey(pubkey.serialize()))
    }

    fn gen_keypair(&self) -> (Self::PrivateKey, Self::PublicKey) {
        let (sk, pubkey) = self.secp.generate_keypair(&mut rand::thread_rng());
        let mut privkey = [0u8; constants::SECRET_KEY_SIZE];
        privkey[..].copy_from_slice(&sk[..]);
        (PrivateKey(privkey), PublicKey(pubkey.serialize()))
    }

    fn sign(
        &self,
        hash: &Hash,
        privkey: &Self::PrivateKey,
    ) -> Result<Self::Signature, CryptoError> {
        let msg = self.msg(hash)?;
        let privkey = SecretKey::from_slice(privkey.as_bytes())
            .map_err(|_| CryptoError::PrivateKeyInvalid)?;

        let (rec_id, data) = self
            .secp
            .sign_recoverable(&msg, &privkey)
            .serialize_compact();

        let mut signature = [0u8; constants::COMPACT_SIGNATURE_SIZE + 1];
        signature[0..64].copy_from_slice(&data[..]);
        signature[signature.len() - 1] = rec_id.to_i32() as u8;
        Ok(Signature(signature))
    }

    fn pubkey_to_address(&self, pubkey: &Self::PublicKey) -> Address {
        let pubkey = RawPublicKey::from_slice(&pubkey.0).expect("should never failed");
        let pubkey_hash = Hash::digest(&pubkey.serialize_uncompressed()[1..]);
        Address::from_hash(&pubkey_hash)
    }
}

impl Secp256k1 {
    pub fn new() -> Self {
        Secp256k1 {
            secp: RawSecp256k1::new(),
        }
    }

    fn verify(
        &self,
        msg: &Message,
        sig: &RecoverableSignature,
        pubkey: &RawPublicKey,
    ) -> Result<(), CryptoError> {
        self.secp
            .verify(&msg, &sig.to_standard(), &pubkey)
            .map_err(|_| CryptoError::SignatureInvalid)?;
        Ok(())
    }

    fn recover(
        &self,
        msg: &Message,
        sig: &RecoverableSignature,
    ) -> Result<RawPublicKey, CryptoError> {
        let pubkey = self
            .secp
            .recover(&msg, &sig)
            .map_err(|_| CryptoError::SignatureInvalid)?;
        Ok(pubkey)
    }

    fn signature(&self, signature: &[u8]) -> Result<RecoverableSignature, CryptoError> {
        if signature.len() != constants::UNCOMPRESSED_PUBLIC_KEY_SIZE {
            return Err(CryptoError::SignatureInvalid);
        }

        let sig = RecoverableSignature::from_compact(
            &signature[0..64],
            RecoveryId::from_i32(i32::from(signature[64]))
                .map_err(|_| CryptoError::SignatureInvalid)?,
        )
        .map_err(|_| CryptoError::SignatureInvalid)?;

        Ok(sig)
    }

    fn msg(&self, hash: &Hash) -> Result<Message, CryptoError> {
        Ok(Message::from_slice(hash.as_bytes()).map_err(|_| CryptoError::SignatureInvalid)?)
    }
}

impl Default for Secp256k1 {
    fn default() -> Self {
        Secp256k1 {
            secp: RawSecp256k1::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Secp256k1;

    use crate::{Crypto, CryptoTransform};
    use core_types::Hash;

    #[test]
    fn test_secp256k1_basic() {
        let secp = Secp256k1::new();
        let (privkey, pubkey) = secp.gen_keypair();

        let test_hash = Hash::digest(b"test");

        // test signature
        let signature = secp.sign(&test_hash, &privkey).unwrap();

        // test verify signature
        let pubkey2 = secp.verify_with_signature(&test_hash, &signature).unwrap();
        assert_eq!(pubkey.as_bytes(), pubkey2.as_bytes());

        // test recover
        let pubkey3 = secp.get_public_key(&privkey).unwrap();
        assert_eq!(pubkey.as_bytes(), pubkey3.as_bytes());
    }
}
