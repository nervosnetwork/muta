use muta_codec_derive::RlpFixedCodec;
use serde::{Deserialize, Serialize};

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
// use protocol::traits::Witness;
use protocol::types::{Address, Bytes, Hex, PubkeyWithSender, TypesError};
use protocol::ProtocolResult;

pub const MAX_PERMISSION_ACCOUNTS: u8 = 16;
const SINGLE_SIGNATURE_WITNESS: u8 = 0;
const MULTI_SIGNATURE_WITNESS: u8 = 1;

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct GetMultiSigAccountPayload {
    pub multi_sig_address: Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default)]
pub struct GetMultiSigAccountResponse {
    pub permission: MultiSigPermission,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct GenerateMultiSigAccountPayload {
    pub owner:     Address,
    pub accounts:  Vec<MultiSigAccount>,
    pub threshold: u32,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default)]
pub struct GenerateMultiSigAccountResponse {
    pub address: Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct VerifyMultiSigPayload {
    pub pubkeys:    Vec<Bytes>,
    pub signatures: Vec<Bytes>,
    pub sender:     Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct ChangeOwnerPayload {
    pub witness:           VerifyMultiSigPayload,
    pub multi_sig_address: Address,
    pub new_owner:         Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct AddAccountPayload {
    pub witness:           VerifyMultiSigPayload,
    pub multi_sig_address: Address,
    pub new_account:       MultiSigAccount,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct RemoveAccountPayload {
    pub witness:           VerifyMultiSigPayload,
    pub multi_sig_address: Address,
    pub account_address:   Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct SetAccountWeightPayload {
    pub witness:           VerifyMultiSigPayload,
    pub multi_sig_address: Address,
    pub account_address:   Address,
    pub new_weight:        u8,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct SetThresholdPayload {
    pub witness:           VerifyMultiSigPayload,
    pub multi_sig_address: Address,
    pub new_threshold:     u32,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default)]
pub struct MultiSigPermission {
    pub owner:     Address,
    pub accounts:  Vec<MultiSigAccount>,
    pub threshold: u32,
}

impl MultiSigPermission {
    pub fn get_account(&self, addr: &Address) -> Option<MultiSigAccount> {
        for account in self.accounts.iter() {
            if &account.address == addr {
                return Some(account.clone());
            }
        }
        None
    }

    pub fn set_owner(&mut self, new_owner: Address) {
        self.owner = new_owner;
    }

    pub fn add_account(&mut self, new_account: MultiSigAccount) {
        self.accounts.push(new_account);
    }

    pub fn remove_account(&mut self, address: &Address) -> Option<MultiSigAccount> {
        let mut idx = self.accounts.len();
        for (index, account) in self.accounts.iter().enumerate() {
            if &account.address == address {
                idx = index;
                break;
            }
        }

        if idx != self.accounts.len() {
            Some(self.accounts.remove(idx))
        } else {
            None
        }
    }

    pub fn set_threshold(&mut self, new_threshold: u32) {
        self.threshold = new_threshold;
    }

    pub fn set_account_weight(&mut self, account_address: &Address, new_weight: u8) -> Option<u8> {
        for account in self.accounts.iter_mut() {
            if &account.address == account_address {
                let ret = account.weight;
                account.weight = new_weight;
                return Some(ret);
            }
        }
        None
    }
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default)]
pub struct MultiSigAccount {
    pub address: Address,
    pub weight:  u8,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct Witness {
    pub pubkeys:    Vec<Bytes>,
    pub signatures: Vec<Bytes>,
}

impl Witness {
    pub fn new(pubkeys: Vec<Bytes>, signatures: Vec<Bytes>) -> Self {
        Witness {
            pubkeys,
            signatures,
        }
    }
}

// #[derive(Deserialize, Serialize, Clone, Debug)]
// struct WitnessInner {
//     pubkey:    Bytes,
//     signature: Bytes,
//     sender:    Option<Address>,
// }

// impl WitnessInner {
//     fn new(pubkey_with_sender: PubkeyWithSender, signature: Bytes) -> Self {
//         WitnessInner {
//             pubkey: pubkey_with_sender.pubkey,
//             sender: pubkey_with_sender.sender,
//             signature,
//         }
//     }
// }

// impl rlp::Encodable for WitnessInner {
//     fn rlp_append(&self, s: &mut rlp::RlpStream) {
//         s.begin_list(4)
//             .append(&self.pubkey.to_vec())
//             .append(&self.signature.to_vec());
//         if let Some(addr) = &self.sender {
//             s.append(&true).append(addr);
//         } else {
//             s.append(&false).append_empty_data();
//         }
//     }
// }

// impl rlp::Decodable for WitnessInner {
//     fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
//         match r.prototype()? {
//             rlp::Prototype::List(4) => {
//                 let pubkey: Vec<u8> = r.val_at(0)?;
//                 let signature: Vec<u8> = r.val_at(1)?;
//                 let flag: bool = r.val_at(2)?;
//                 let sender = if flag {
//                     let addr: Address = r.val_at(3)?;
//                     Some(addr)
//                 } else {
//                     None
//                 };

//                 Ok(WitnessInner {
//                     pubkey: Bytes::from(pubkey),
//                     signature: Bytes::from(signature),
//                     sender,
//                 })
//             }
//             _ => Err(rlp::DecoderError::RlpInconsistentLengthAndData),
//         }
//     }
// }

// impl FixedCodec for WitnessInner {
//     fn encode_fixed(&self) -> ProtocolResult<Bytes> {
//         Ok(Bytes::from(rlp::encode(self)))
//     }

//     fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
//         Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
//     }
// }
