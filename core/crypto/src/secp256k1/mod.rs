use secp256k1::{
    constants,
    key::{PublicKey, SecretKey},
    rand, Message, RecoverableSignature, RecoveryId, Secp256k1 as RawSecp256k1,
};

use core_types::Hash;

use crate::{Crypto, CryptoError, Keypair};

pub struct Secp256k1;

impl Crypto for Secp256k1 {
    fn recover_public_key(hash: &Hash, signature: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let msg = Secp256k1::msg(hash)?;
        let sig = Secp256k1::signature(signature)?;
        let pubkey = Secp256k1::recover(&msg, &sig)?;

        Ok(pubkey.serialize().to_vec())
    }

    fn verify_with_signature(hash: &Hash, signature: &[u8]) -> Result<(), CryptoError> {
        let msg = Secp256k1::msg(hash)?;
        let sig = Secp256k1::signature(signature)?;
        let pubkey = Secp256k1::recover(&msg, &sig)?;

        Secp256k1::verify(&msg, &sig, &pubkey)?;
        Ok(())
    }

    fn gen_keypair() -> Keypair {
        let (privkey, pubkey) = RawSecp256k1::new().generate_keypair(&mut rand::thread_rng());

        Keypair {
            private_key: privkey[..].to_vec(),
            public_key: pubkey.serialize().to_vec(),
        }
    }

    fn sign(hash: &Hash, privkey: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let msg = Secp256k1::msg(hash)?;
        let privkey = SecretKey::from_slice(privkey).map_err(|_| CryptoError::PrivateKeyInvalid)?;

        let secp = RawSecp256k1::new();
        let (rec_id, data) = secp.sign_recoverable(&msg, &privkey).serialize_compact();

        let mut signature = [0u8; constants::COMPACT_SIGNATURE_SIZE + 1];
        signature[0..64].copy_from_slice(&data[..]);
        signature[signature.len() - 1] = rec_id.to_i32() as u8;
        Ok(signature.to_vec())
    }
}

impl Secp256k1 {
    fn verify(
        msg: &Message,
        sig: &RecoverableSignature,
        pubkey: &PublicKey,
    ) -> Result<(), CryptoError> {
        let secp = RawSecp256k1::new();
        secp.verify(&msg, &sig.to_standard(), &pubkey)
            .map_err(|_| CryptoError::SignatureInvalid)?;
        Ok(())
    }

    fn recover(msg: &Message, sig: &RecoverableSignature) -> Result<PublicKey, CryptoError> {
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

    use crate::Crypto;
    use core_types::Hash;

    #[test]
    fn test_secp256k1_basic() {
        let keypair = Secp256k1::gen_keypair();

        let test_hash = Hash::from_raw(b"test");

        // test signature
        let signature = Secp256k1::sign(&test_hash, &keypair.private_key).unwrap();

        // test verify signature
        Secp256k1::verify_with_signature(&test_hash, &signature).unwrap();

        // test recover
        let pubkey = Secp256k1::recover_public_key(&test_hash, &signature).unwrap();
        assert_eq!(keypair.public_key, pubkey)
    }
}
