#![allow(clippy::suspicious_else_formatting, clippy::mutable_key_type)]

#[cfg(test)]
mod tests;
pub mod types;

use std::collections::HashMap;

use binding_macro::{cycles, genesis, service};
use derive_more::Display;
use rlp::{Decodable, Rlp};

use common_crypto::{Crypto, Secp256k1};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{Address, Bytes, Hash, ServiceContext, SignedTransaction};

use crate::types::{
    Account, AddAccountPayload, ChangeMemoPayload, ChangeOwnerPayload,
    GenerateMultiSigAccountPayload, GenerateMultiSigAccountResponse, GetMultiSigAccountPayload,
    GetMultiSigAccountResponse, InitGenesisPayload, MultiSigPermission, RemoveAccountPayload,
    RemoveAccountResult, SetAccountWeightPayload, SetThresholdPayload, SetWeightResult,
    UpdateAccountPayload, VerifySignaturePayload, Witness,
};

pub const MULTI_SIG_SERVICE_NAME: &str = "multi_signature";
const MAX_MULTI_SIGNATURE_RECURSION_DEPTH: u8 = 8;
const MAX_PERMISSION_ACCOUNTS: u8 = 16;

pub trait MultiSignature {
    fn verify_signature_(
        &self,
        ctx: &ServiceContext,
        payload: SignedTransaction,
    ) -> ServiceResponse<()>;

    fn generate_account_(
        &mut self,
        ctx: &ServiceContext,
        payload: GenerateMultiSigAccountPayload,
    ) -> ServiceResponse<GenerateMultiSigAccountResponse>;
}

pub struct MultiSignatureService<SDK> {
    sdk: SDK,
}

impl<SDK: ServiceSDK> MultiSignature for MultiSignatureService<SDK> {
    fn verify_signature_(
        &self,
        ctx: &ServiceContext,
        payload: SignedTransaction,
    ) -> ServiceResponse<()> {
        self.verify_signature(ctx.clone(), payload)
    }

    fn generate_account_(
        &mut self,
        ctx: &ServiceContext,
        payload: GenerateMultiSigAccountPayload,
    ) -> ServiceResponse<GenerateMultiSigAccountResponse> {
        self.generate_account(ctx.clone(), payload)
    }
}

#[service]
impl<SDK: ServiceSDK> MultiSignatureService<SDK> {
    pub fn new(sdk: SDK) -> Self {
        MultiSignatureService { sdk }
    }

    #[genesis]
    fn init_genesis(&mut self, payload: InitGenesisPayload) {
        if payload.addr_with_weight.is_empty()
            || payload.addr_with_weight.len() > MAX_PERMISSION_ACCOUNTS as usize
        {
            panic!("Invalid account number");
        }

        let weight_sum = payload
            .addr_with_weight
            .iter()
            .map(|item| item.weight as u32)
            .sum::<u32>();

        if payload.threshold == 0 || weight_sum < payload.threshold {
            panic!("Invalid threshold or weights");
        }

        let address = payload.address.clone();
        let accounts = payload
            .addr_with_weight
            .iter()
            .map(|item| Account {
                address:     item.address.clone(),
                weight:      item.weight,
                is_multiple: false,
            })
            .collect::<Vec<_>>();

        let permission = MultiSigPermission {
            accounts,
            owner: payload.owner,
            threshold: payload.threshold,
            memo: payload.memo,
        };

        self.sdk.set_account_value(&address, 0u8, permission);
    }

    #[cycles(21_000)]
    #[write]
    fn generate_account(
        &mut self,
        ctx: ServiceContext,
        payload: GenerateMultiSigAccountPayload,
    ) -> ServiceResponse<GenerateMultiSigAccountResponse> {
        if payload.addr_with_weight.is_empty()
            || payload.addr_with_weight.len() > MAX_PERMISSION_ACCOUNTS as usize
        {
            return ServiceError::InvalidAccountLength.into();
        }

        let weight_sum = payload
            .addr_with_weight
            .iter()
            .map(|item| item.weight as u32)
            .sum::<u32>();

        if payload.threshold == 0 || weight_sum < payload.threshold {
            return ServiceError::InvalidAccountWeights.into();
        }

        // check the recursion depth
        if payload
            .addr_with_weight
            .iter()
            .map(|s| self._is_recursion_depth_overflow(&s.address, 0))
            .any(|res| res)
        {
            return ServiceError::AboveMaxRecursionDepth.into();
        }

        let tx_hash = match ctx.get_tx_hash() {
            Some(hash) => hash,
            None => return ServiceError::CtxMissingTxHash.into(),
        };

        if let Ok(address) = Address::from_hash(Hash::digest(tx_hash.as_bytes())) {
            let accounts = payload
                .addr_with_weight
                .iter()
                .map(|item| Account {
                    address:     item.address.clone(),
                    weight:      item.weight,
                    is_multiple: !self
                        .get_account_from_address(ctx.clone(), GetMultiSigAccountPayload {
                            multi_sig_address: item.address.clone(),
                        })
                        .is_error(),
                })
                .collect::<Vec<_>>();

            let owner = if payload.autonomy {
                address.clone()
            } else {
                payload.owner.clone()
            };

            let permission = MultiSigPermission {
                accounts,
                owner,
                threshold: payload.threshold,
                memo: payload.memo,
            };

            self.sdk.set_account_value(&address, 0u8, permission);
            ServiceResponse::<GenerateMultiSigAccountResponse>::from_succeed(
                GenerateMultiSigAccountResponse { address },
            )
        } else {
            ServiceError::GenerateAddressFailed.into()
        }
    }

    #[cycles(10_000)]
    #[read]
    fn get_account_from_address(
        &self,
        _ctx: ServiceContext,
        payload: GetMultiSigAccountPayload,
    ) -> ServiceResponse<GetMultiSigAccountResponse> {
        if let Some(permission) = self.sdk.get_account_value(&payload.multi_sig_address, &0u8) {
            ServiceResponse::<GetMultiSigAccountResponse>::from_succeed(
                GetMultiSigAccountResponse { permission },
            )
        } else {
            ServiceError::AccountNotExsit.into()
        }
    }

    #[cycles(21_000)]
    #[read]
    pub fn verify_signature(
        &self,
        ctx: ServiceContext,
        payload: SignedTransaction,
    ) -> ServiceResponse<()> {
        let pubkeys = match decode_list::<Vec<u8>>(&payload.pubkey, "public key") {
            Ok(pks) => pks,
            Err(err) => return err.into(),
        };

        let sigs = match decode_list::<Vec<u8>>(&payload.signature, "signature") {
            Ok(sig) => sig,
            Err(err) => return err.into(),
        };

        self._inner_verify_signature(VerifySignaturePayload {
            tx_hash:    payload.tx_hash,
            pubkeys:    pubkeys.into_iter().map(Bytes::from).collect::<Vec<_>>(),
            signatures: sigs.into_iter().map(Bytes::from).collect::<Vec<_>>(),
            sender:     payload.raw.sender,
        })
    }

    #[cycles(21_000)]
    #[write]
    fn update_account(
        &mut self,
        ctx: ServiceContext,
        payload: UpdateAccountPayload,
    ) -> ServiceResponse<()> {
        if let Some(permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(&payload.account_address, &0u8)
        {
            // check owner address
            if ctx.get_caller() != permission.owner {
                return ServiceError::InvalidOwner.into();
            }

            // check if account contains itself
            if payload
                .addr_with_weight
                .iter()
                .map(|a| a.address.clone())
                .any(|addr| addr == payload.account_address)
            {
                return ServiceError::AccountSelfContained.into();
            }

            // check sum of weight
            if payload.addr_with_weight.is_empty()
                || payload.addr_with_weight.len() > MAX_PERMISSION_ACCOUNTS as usize
            {
                return ServiceError::InvalidAccountLength.into();
            }

            let weight_sum = payload
                .addr_with_weight
                .iter()
                .map(|item| item.weight as u32)
                .sum::<u32>();

            // check if sum of the weights is above threshold
            if payload.threshold == 0 || weight_sum < payload.threshold {
                return ServiceError::InvalidAccountWeights.into();
            }

            // check the recursion depth
            if payload
                .addr_with_weight
                .iter()
                .map(|s| self._is_recursion_depth_overflow(&s.address, 0))
                .any(|res| res)
            {
                return ServiceError::AboveMaxRecursionDepth.into();
            }

            let accounts = payload
                .addr_with_weight
                .iter()
                .map(|item| Account {
                    address:     item.address.clone(),
                    weight:      item.weight,
                    is_multiple: !self
                        .get_account_from_address(ctx.clone(), GetMultiSigAccountPayload {
                            multi_sig_address: item.address.clone(),
                        })
                        .is_error(),
                })
                .collect::<Vec<_>>();

            self.sdk
                .set_account_value(&payload.account_address, 0u8, MultiSigPermission {
                    accounts,
                    owner: payload.owner,
                    threshold: payload.threshold,
                    memo: payload.memo,
                });
            return ServiceResponse::<()>::from_succeed(());
        }

        ServiceError::AccountNotExsit.into()
    }

    #[cycles(21_000)]
    #[write]
    fn change_owner(
        &mut self,
        ctx: ServiceContext,
        payload: ChangeOwnerPayload,
    ) -> ServiceResponse<()> {
        if let Some(mut permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(&payload.multi_sig_address, &0u8)
        {
            // check owner address
            if ctx.get_caller() != permission.owner {
                return ServiceError::InvalidOwner.into();
            }

            // check new owner's recursion depth
            if self._is_recursion_depth_overflow(&payload.new_owner, 0) {
                return ServiceError::AboveMaxRecursionDepth.into();
            }

            permission.set_owner(payload.new_owner);
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceError::AccountNotExsit.into()
        }
    }

    #[cycles(21_000)]
    #[write]
    fn change_memo(
        &mut self,
        ctx: ServiceContext,
        payload: ChangeMemoPayload,
    ) -> ServiceResponse<()> {
        if let Some(mut permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(&payload.multi_sig_address, &0u8)
        {
            // check owner address
            if ctx.get_caller() != permission.owner {
                return ServiceError::InvalidOwner.into();
            }

            permission.set_memo(payload.new_memo);
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceError::AccountNotExsit.into()
        }
    }

    #[cycles(21_000)]
    #[write]
    fn add_account(
        &mut self,
        ctx: ServiceContext,
        payload: AddAccountPayload,
    ) -> ServiceResponse<()> {
        if let Some(mut permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(&payload.multi_sig_address, &0u8)
        {
            // check owner address
            if ctx.get_caller() != permission.owner {
                return ServiceError::InvalidOwner.into();
            }

            // check whether reach the max count
            if permission.accounts.len() == MAX_PERMISSION_ACCOUNTS as usize {
                return ServiceError::AccountCountReachMaxValue.into();
            }

            // check whether the new account above max recursion depth
            if self._is_recursion_depth_overflow(&payload.new_account.address, 1) {
                return ServiceError::AboveMaxRecursionDepth.into();
            }

            permission.add_account(payload.new_account.clone());
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);

            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceError::AccountNotExsit.into()
        }
    }

    #[cycles(21_000)]
    #[write]
    fn remove_account(
        &mut self,
        ctx: ServiceContext,
        payload: RemoveAccountPayload,
    ) -> ServiceResponse<Account> {
        if let Some(mut permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(&payload.multi_sig_address, &0u8)
        {
            // check owner address
            if ctx.get_caller() != permission.owner {
                return ServiceError::InvalidOwner.into();
            }

            match permission.remove_account(&payload.account_address) {
                RemoveAccountResult::Success(ret) => {
                    self.sdk
                        .set_account_value(&payload.multi_sig_address, 0u8, permission);
                    return ServiceResponse::<Account>::from_succeed(ret);
                }
                RemoveAccountResult::BelowThreshold => {
                    return ServiceError::InvalidAccountWeights.into();
                }
                _ => (),
            }
        }
        ServiceError::AccountNotExsit.into()
    }

    #[cycles(21_000)]
    #[write]
    fn set_account_weight(
        &mut self,
        ctx: ServiceContext,
        payload: SetAccountWeightPayload,
    ) -> ServiceResponse<()> {
        if let Some(mut permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(&payload.multi_sig_address, &0u8)
        {
            // check owner address
            if ctx.get_caller() != permission.owner {
                return ServiceError::InvalidOwner.into();
            }

            match permission.set_account_weight(&payload.account_address, payload.new_weight) {
                SetWeightResult::Success => {
                    self.sdk
                        .set_account_value(&payload.multi_sig_address, 0u8, permission);
                    return ServiceResponse::<()>::from_succeed(());
                }
                SetWeightResult::InvalidNewWeight => {
                    return ServiceError::InvalidAccountWeights.into();
                }
                _ => (),
            }
        }
        ServiceError::AccountNotExsit.into()
    }

    #[cycles(21_000)]
    #[write]
    fn set_threshold(
        &mut self,
        ctx: ServiceContext,
        payload: SetThresholdPayload,
    ) -> ServiceResponse<()> {
        if let Some(mut permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(&payload.multi_sig_address, &0u8)
        {
            // check owner address
            if ctx.get_caller() != permission.owner {
                return ServiceError::InvalidOwner.into();
            }

            // check new threshold
            if permission
                .accounts
                .iter()
                .map(|account| account.weight as u32)
                .sum::<u32>()
                < payload.new_threshold
            {
                return ServiceError::InvalidAccountWeights.into();
            }

            permission.set_threshold(payload.new_threshold);
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceError::AccountNotExsit.into()
        }
    }

    fn _inner_verify_signature(&self, payload: VerifySignaturePayload) -> ServiceResponse<()> {
        if payload.pubkeys.len() != payload.signatures.len() {
            return ServiceError::PubkeyAndSignatureMismatch.into();
        }

        if payload.pubkeys.len() == 1 {
            if let Ok(addr) = Address::from_pubkey_bytes(&payload.pubkeys[0]) {
                if addr == payload.sender {
                    return self._verify_single_signature(
                        &payload.tx_hash,
                        &payload.signatures[0],
                        &payload.pubkeys[0],
                    );
                }
            } else {
                return ServiceError::InvalidPublicKey.into();
            }
        }

        self._verify_multi_signature(
            &payload.tx_hash,
            &Witness::new(payload.pubkeys, payload.signatures).into_addr_map(),
            &payload.sender,
            0u8,
        )
    }

    fn _verify_multi_signature(
        &self,
        tx_hash: &Hash,
        wit_map: &HashMap<Address, (Bytes, Bytes)>,
        sender: &Address,
        recursion_depth: u8,
    ) -> ServiceResponse<()> {
        // use local variable to do DFS
        let depth_clone = recursion_depth + 1;

        // check recursion depth
        if depth_clone >= MAX_MULTI_SIGNATURE_RECURSION_DEPTH {
            return ServiceError::AboveMaxRecursionDepth.into();
        }

        let mut weight_acc = 0u32;

        let permission = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(sender, &0u8);
        if permission.is_none() {
            return ServiceError::AccountNotExsit.into();
        }
        let permission = permission.unwrap();

        for account in permission.accounts.iter() {
            if !account.is_multiple {
                if let Some((pk, sig)) = wit_map.get(&account.address) {
                    if !self._verify_single_signature(tx_hash, sig, pk).is_error() {
                        weight_acc += account.weight as u32;
                    }
                }
            } else if !self
                ._verify_multi_signature(tx_hash, wit_map, &account.address, depth_clone)
                .is_error()
            {
                weight_acc += account.weight as u32;
            }

            if weight_acc >= permission.threshold {
                return ServiceResponse::<()>::from_succeed(());
            }
        }

        ServiceError::VerifyMultiSignatureFailed.into()
    }

    fn _verify_single_signature(
        &self,
        tx_hash: &Hash,
        sig: &Bytes,
        pubkey: &Bytes,
    ) -> ServiceResponse<()> {
        if Secp256k1::verify_signature(tx_hash.as_slice(), sig.as_ref(), pubkey.as_ref()).is_ok() {
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceError::VerifyMultiSignatureFailed.into()
        }
    }

    fn _is_recursion_depth_overflow(&self, address: &Address, recursion_depth: u8) -> bool {
        let depth_clone = recursion_depth + 1;
        if depth_clone >= MAX_MULTI_SIGNATURE_RECURSION_DEPTH {
            return true;
        }

        if let Some(permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(address, &0u8)
        {
            permission
                .accounts
                .iter()
                .filter(|account| account.is_multiple)
                .map(|account| self._is_recursion_depth_overflow(&account.address, depth_clone))
                .any(|overflow| overflow)
        } else {
            false
        }
    }
}

#[derive(Debug, Display)]
pub enum ServiceError {
    #[display(fmt = "Decode {:?} error", _0)]
    DecodeErr(String),

    #[display(fmt = "accounts length must be [1,16]")]
    InvalidAccountLength,

    #[display(fmt = "accounts weight or threshold not valid")]
    InvalidAccountWeights,

    #[display(fmt = "above max recursion depth")]
    AboveMaxRecursionDepth,

    #[display(fmt = "Can not get tx hash from service context")]
    CtxMissingTxHash,

    #[display(fmt = "generate address from tx_hash failed")]
    GenerateAddressFailed,

    #[display(fmt = "account is not existed")]
    AccountNotExsit,

    #[display(fmt = "invalid owner")]
    InvalidOwner,

    #[display(fmt = "account can not contain itself")]
    AccountSelfContained,

    #[display(fmt = "the account count reach max value")]
    AccountCountReachMaxValue,

    #[display(fmt = "pubkkeys len is not equal to signatures len")]
    PubkeyAndSignatureMismatch,

    #[display(fmt = "invalid public key")]
    InvalidPublicKey,

    #[display(fmt = "multi signature verified failed")]
    VerifyMultiSignatureFailed,
}

impl ServiceError {
    fn code(&self) -> u64 {
        match self {
            ServiceError::DecodeErr(_) => 101,
            ServiceError::InvalidAccountLength => 102,
            ServiceError::InvalidAccountWeights => 103,
            ServiceError::AboveMaxRecursionDepth => 104,
            ServiceError::CtxMissingTxHash => 105,
            ServiceError::GenerateAddressFailed => 106,
            ServiceError::AccountNotExsit => 107,
            ServiceError::InvalidOwner => 108,
            ServiceError::AccountSelfContained => 109,
            ServiceError::AccountCountReachMaxValue => 110,
            ServiceError::PubkeyAndSignatureMismatch => 111,
            ServiceError::InvalidPublicKey => 112,
            ServiceError::VerifyMultiSignatureFailed => 113,
        }
    }
}

impl<T: Default> From<ServiceError> for ServiceResponse<T> {
    fn from(err: ServiceError) -> ServiceResponse<T> {
        ServiceResponse::from_error(err.code(), err.to_string())
    }
}

fn decode_list<T: Decodable>(bytes: &[u8], ty: &str) -> Result<Vec<T>, ServiceError> {
    Rlp::new(bytes)
        .as_list()
        .map_err(|_| ServiceError::DecodeErr(ty.to_string()))
}
