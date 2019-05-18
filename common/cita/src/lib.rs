use std::convert::TryInto;
use std::sync::Arc;

use core_crypto::{Crypto as CoreCrypto, CryptoError, CryptoTransform};
use core_serialization::{generate_module_for, SyncCodec};
use core_types::{self, Address, Hash};

generate_module_for!([blockchain]);

impl TryInto<core_types::Transaction> for Transaction {
    type Error = core_types::TypesError;

    fn try_into(self) -> Result<core_types::Transaction, Self::Error> {
        let to = match self.version {
            1 => {
                if self.to_v1.is_empty() {
                    None
                } else {
                    Some(Address::from_bytes(&self.to_v1)?)
                }
            }
            _ => {
                if self.to.is_empty() {
                    None
                } else {
                    Some(Address::from_hex(&self.to)?)
                }
            }
        };
        let chain_id = match self.version {
            1 => self.chain_id_v1,
            _ => self.chain_id.to_be_bytes().to_vec(),
        };
        Ok(core_types::Transaction {
            to,
            nonce: self.nonce,
            quota: self.quota,
            valid_until_block: self.valid_until_block,
            data: self.data,
            value: self.value,
            chain_id,
        })
    }
}

impl From<core_types::Transaction> for Transaction {
    fn from(tx: core_types::Transaction) -> Self {
        let version = if tx.chain_id.len() <= 4 { 0 } else { 1 };
        Self {
            to: if version == 0 {
                tx.to
                    .clone()
                    .and_then(|s| Some(s.as_checksum_hex()))
                    .unwrap_or_default()
            } else {
                String::new()
            },
            nonce: tx.nonce.clone(),
            quota: tx.quota,
            valid_until_block: tx.valid_until_block,
            data: tx.data.clone(),
            value: tx.value.clone(),
            chain_id: if version == 0 {
                let mut a: [u8; 4] = Default::default();
                a.copy_from_slice(&tx.chain_id[0..4]);
                u32::from_be_bytes(a)
            } else {
                0
            },
            version,
            to_v1: if version == 1 {
                tx.to
                    .and_then(|s| Some(s.as_bytes().to_vec()))
                    .unwrap_or_else(|| vec![])
            } else {
                vec![]
            },
            chain_id_v1: if version == 1 { tx.chain_id } else { vec![] },
        }
    }
}

impl Transaction {
    pub fn hash(&self) -> Hash {
        let ser_raw_tx = SyncCodec::encode(self.clone()).unwrap();
        Hash::from_fixed_bytes(tiny_keccak::keccak256(&ser_raw_tx))
    }
}

impl TryInto<core_types::UnverifiedTransaction> for UnverifiedTransaction {
    type Error = core_types::TypesError;

    fn try_into(self) -> Result<core_types::UnverifiedTransaction, Self::Error> {
        Ok(core_types::UnverifiedTransaction {
            transaction: self.transaction.unwrap_or_default().try_into()?,
            signature:   self.signature,
        })
    }
}

impl From<core_types::UnverifiedTransaction> for UnverifiedTransaction {
    fn from(untx: core_types::UnverifiedTransaction) -> Self {
        Self {
            transaction: Some(untx.transaction.into()),
            signature:   untx.signature,
            crypto:      0,
        }
    }
}

impl UnverifiedTransaction {
    pub fn verify<C: CoreCrypto + 'static>(&self, c: Arc<C>) -> Result<C::PublicKey, CryptoError> {
        let signature = C::Signature::from_bytes(&self.signature)?;
        c.verify_with_signature(&self.transaction.clone().unwrap().hash(), &signature)
    }
}
