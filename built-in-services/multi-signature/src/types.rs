use std::collections::HashMap;

use muta_codec_derive::RlpFixedCodec;
use serde::{Deserialize, Serialize};

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::types::{Address, Bytes, Hash};
use protocol::ProtocolResult;

#[derive(Clone, Debug)]
pub enum SetWeightResult {
    Success,
    NoAccount,
    InvalidNewWeight,
}

#[derive(Clone, Debug)]
pub enum RemoveAccountResult {
    Success(Account),
    NoAccount,
    BelowThreshold,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct InitGenesisPayload {
    pub address:          Address,
    pub owner:            Address,
    pub addr_with_weight: Vec<AddressWithWeight>,
    pub threshold:        u32,
    pub memo:             String,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct GenerateMultiSigAccountPayload {
    pub owner:            Address,
    pub addr_with_weight: Vec<AddressWithWeight>,
    pub threshold:        u32,
    pub memo:             String,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default)]
pub struct GenerateMultiSigAccountResponse {
    pub address: Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct VerifySignaturePayload {
    pub tx_hash:    Hash,
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
pub struct ChangeMemoPayload {
    pub witness:           VerifySignaturePayload,
    pub multi_sig_address: Address,
    pub new_memo:          String,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct AddAccountPayload {
    pub witness:           VerifySignaturePayload,
    pub multi_sig_address: Address,
    pub new_account:       Account,
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
    pub accounts:  Vec<Account>,
    pub threshold: u32,
    pub memo:      String,
}

impl MultiSigPermission {
    pub fn get_account(&self, addr: &Address) -> Option<Account> {
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

    pub fn set_memo(&mut self, new_memo: String) {
        self.memo = new_memo;
    }

    pub fn add_account(&mut self, new_account: Account) {
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
pub struct Account {
    pub address:     Address,
    pub weight:      u8,
    pub is_multiple: bool,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AddressWithWeight {
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

    pub fn to_addr_map(&self) -> HashMap<Address, (Bytes, Bytes)> {
        let mut ret = HashMap::new();
        for (pk, sig) in self.pubkeys.iter().zip(self.signatures.iter()) {
            if let Ok(addr) = Address::from_pubkey_bytes(pk.clone()) {
                ret.insert(addr, (pk.clone(), sig.clone()));
            }
        }
        ret
    }
}

#[cfg(test)]
impl AddressWithWeight {
    pub fn into_signle_account(self) -> Account {
        Account {
            address:     self.address,
            weight:      self.weight,
            is_multiple: false,
        }
    }
}
