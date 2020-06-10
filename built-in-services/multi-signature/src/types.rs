use muta_codec_derive::RlpFixedCodec;
use serde::{Deserialize, Serialize};

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::types::{Address, Bytes};
use protocol::ProtocolResult;

pub const MAX_PERMISSION_ACCOUNTS: u8 = 16;
#[derive(Clone, Debug)]
pub enum SetWeightResult {
    Success,
    NoAccount,
    InvalidNewWeight,
}

#[derive(Clone, Debug)]
pub enum RemoveAccountResult {
    Success(MultiSigAccount),
    NoAccount,
    BelowThreshold,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
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
pub struct VerifySignaturePayload {
    pub pubkeys:    Vec<Bytes>,
    pub signatures: Vec<Bytes>,
    pub sender:     Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct GetMultiSigAccountPayload {
    pub multi_sig_address: Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default)]
pub struct GetMultiSigAccountResponse {
    pub permission: MultiSigPermission,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct ChangeOwnerPayload {
    pub witness:           VerifySignaturePayload,
    pub multi_sig_address: Address,
    pub new_owner:         Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct AddAccountPayload {
    pub witness:           VerifySignaturePayload,
    pub multi_sig_address: Address,
    pub new_account:       MultiSigAccount,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct RemoveAccountPayload {
    pub witness:           VerifySignaturePayload,
    pub multi_sig_address: Address,
    pub account_address:   Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct SetAccountWeightPayload {
    pub witness:           VerifySignaturePayload,
    pub multi_sig_address: Address,
    pub account_address:   Address,
    pub new_weight:        u8,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct SetThresholdPayload {
    pub witness:           VerifySignaturePayload,
    pub multi_sig_address: Address,
    pub new_threshold:     u32,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default, PartialEq, Eq)]
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

    pub fn remove_account(&mut self, address: &Address) -> RemoveAccountResult {
        let mut idx = self.accounts.len();
        let weight_sum = self
            .accounts
            .iter()
            .map(|account| account.weight as u32)
            .sum::<u32>();

        for (index, account) in self.accounts.iter().enumerate() {
            if &account.address == address {
                idx = index;
                break;
            }
        }

        if idx != self.accounts.len() {
            if (weight_sum - self.accounts[idx].weight as u32) < self.threshold {
                RemoveAccountResult::BelowThreshold
            } else {
                let ret = self.accounts.remove(idx);
                RemoveAccountResult::Success(ret)
            }
        } else {
            RemoveAccountResult::NoAccount
        }
    }

    pub fn set_threshold(&mut self, new_threshold: u32) {
        self.threshold = new_threshold;
    }

    pub fn set_account_weight(
        &mut self,
        account_address: &Address,
        new_weight: u8,
    ) -> SetWeightResult {
        let weight_sum = self
            .accounts
            .iter()
            .map(|account| account.weight as u32)
            .sum::<u32>();

        for account in self.accounts.iter_mut() {
            if &account.address == account_address {
                if weight_sum + (new_weight as u32) - (account.weight as u32) < self.threshold {
                    return SetWeightResult::InvalidNewWeight;
                } else {
                    account.weight = new_weight;
                    return SetWeightResult::Success;
                }
            }
        }
        SetWeightResult::NoAccount
    }
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default, PartialEq, Eq)]
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
