use secp256k1::{
    constants,
    key::{PublicKey as RawPublicKey, SecretKey},
    rand, Message, RecoverableSignature, RecoveryId, Secp256k1 as RawSecp256k1,
};

use core_types::Hash;

use crate::{Crypto, CryptoError, CryptoTransform};

pub struct Secp256k1;

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

    fn get_public_key(privkey: &Self::PrivateKey) -> Result<Self::PublicKey, CryptoError> {
        let privkey = SecretKey::from_slice(privkey.as_bytes())
            .map_err(|_| CryptoError::PrivateKeyInvalid)?;
        let secp = RawSecp256k1::new();
        let pubkey = RawPublicKey::from_secret_key(&secp, &privkey);
        Ok(PublicKey(pubkey.serialize()))
    }

    fn verify_with_signature(
        hash: &Hash,
        signature: &Self::Signature,
    ) -> Result<Self::PublicKey, CryptoError> {
        let msg = Secp256k1::msg(hash)?;
        let sig = Secp256k1::signature(signature.as_bytes())?;
        let pubkey = Secp256k1::recover(&msg, &sig)?;

        Secp256k1::verify(&msg, &sig, &pubkey)?;
        Ok(PublicKey(pubkey.serialize()))
    }

    fn gen_keypair() -> (Self::PrivateKey, Self::PublicKey) {
        let (sk, pubkey) = RawSecp256k1::new().generate_keypair(&mut rand::thread_rng());
        let mut privkey = [0u8; constants::SECRET_KEY_SIZE];
        privkey[..].copy_from_slice(&sk[..]);
        (PrivateKey(privkey), PublicKey(pubkey.serialize()))
    }

    fn sign(hash: &Hash, privkey: &Self::PrivateKey) -> Result<Self::Signature, CryptoError> {
        let msg = Secp256k1::msg(hash)?;
        let privkey = SecretKey::from_slice(privkey.as_bytes())
            .map_err(|_| CryptoError::PrivateKeyInvalid)?;

        let secp = RawSecp256k1::new();
        let (rec_id, data) = secp.sign_recoverable(&msg, &privkey).serialize_compact();

        let mut signature = [0u8; constants::COMPACT_SIGNATURE_SIZE + 1];
        signature[0..64].copy_from_slice(&data[..]);
        signature[signature.len() - 1] = rec_id.to_i32() as u8;
        Ok(Signature(signature))
    }
}

impl Secp256k1 {
    fn verify(
        msg: &Message,
        sig: &RecoverableSignature,
        pubkey: &RawPublicKey,
    ) -> Result<(), CryptoError> {
        let secp = RawSecp256k1::new();
        secp.verify(&msg, &sig.to_standard(), &pubkey)
            .map_err(|_| CryptoError::SignatureInvalid)?;
        Ok(())
    }

    fn recover(msg: &Message, sig: &RecoverableSignature) -> Result<RawPublicKey, CryptoError> {
        let secp = RawSecp256k1::new();
        let pubkey = secp
            .recover(&msg, &sig)
            .map_err(|_| CryptoError::SignatureInvalid)?;
        Ok(pubkey)
    }

    fn signature(signature: &[u8]) -> Result<RecoverableSignature, CryptoError> {
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

    fn msg(hash: &Hash) -> Result<Message, CryptoError> {
        Ok(Message::from_slice(hash.as_ref()).map_err(|_| CryptoError::SignatureInvalid)?)
    }
}

#[cfg(test)]
mod tests {
    use super::Secp256k1;

    use crate::{Crypto, CryptoTransform};
    use core_types::Hash;

    #[test]
    fn test_secp256k1_basic() {
        let (privkey, pubkey) = Secp256k1::gen_keypair();

        let test_hash = Hash::from_raw(b"test");

        // test signature
        let signature = Secp256k1::sign(&test_hash, &privkey).unwrap();

        // test verify signature
        let pubkey2 = Secp256k1::verify_with_signature(&test_hash, &signature).unwrap();
        assert_eq!(pubkey.as_bytes(), pubkey2.as_bytes());

        // test recover
        let pubkey3 = Secp256k1::get_public_key(&privkey).unwrap();
        assert_eq!(pubkey.as_bytes(), pubkey3.as_bytes());
    }
}
