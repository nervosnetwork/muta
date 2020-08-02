#![allow(clippy::suspicious_else_formatting, clippy::mutable_key_type)]

#[cfg(test)]
mod tests;
pub mod types;

use std::collections::HashMap;
use std::panic::catch_unwind;

use binding_macro::{cycles, genesis, service};

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

const MAX_MULTI_SIGNATURE_RECURSION_DEPTH: u8 = 8;
const MAX_PERMISSION_ACCOUNTS: u8 = 16;

macro_rules! impl_multisig {
    ($self: expr, $method: ident, $ctx: expr) => {{
        let res = $self.$method($ctx.clone());
        if res.is_error() {
            Err(ServiceResponse::from_error(res.code, res.error_message))
        } else {
            Ok(res.succeed_data)
        }
    }};
    ($self: expr, $method: ident, $ctx: expr, $payload: expr) => {{
        let res = $self.$method($ctx.clone(), $payload);
        if res.is_error() {
            Err(ServiceResponse::from_error(res.code, res.error_message))
        } else {
            Ok(res.succeed_data)
        }
    }};
}

pub trait MultiSignature {
    fn verify_signature_(
        &self,
        ctx: &ServiceContext,
        payload: SignedTransaction,
    ) -> Result<(), ServiceResponse<()>>;

    fn generate_account_(
        &mut self,
        ctx: &ServiceContext,
        payload: GenerateMultiSigAccountPayload,
    ) -> Result<GenerateMultiSigAccountResponse, ServiceResponse<()>>;
}

pub struct MultiSignatureService<SDK> {
    sdk: SDK,
}

impl<SDK: ServiceSDK> MultiSignature for MultiSignatureService<SDK> {
    fn verify_signature_(
        &self,
        ctx: &ServiceContext,
        payload: SignedTransaction,
    ) -> Result<(), ServiceResponse<()>> {
        impl_multisig!(self, verify_signature, ctx, payload)
    }

    fn generate_account_(
        &mut self,
        ctx: &ServiceContext,
        payload: GenerateMultiSigAccountPayload,
    ) -> Result<GenerateMultiSigAccountResponse, ServiceResponse<()>> {
        impl_multisig!(self, generate_account, ctx, payload)
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

        let owner = payload.owner;

        let permission = MultiSigPermission {
            accounts,
            owner,
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
            return ServiceResponse::<GenerateMultiSigAccountResponse>::from_error(
                110,
                "accounts length must be [1,16]".to_owned(),
            );
        }

        let weight_sum = payload
            .addr_with_weight
            .iter()
            .map(|item| item.weight as u32)
            .sum::<u32>();

        if payload.threshold == 0 || weight_sum < payload.threshold {
            return ServiceResponse::<GenerateMultiSigAccountResponse>::from_error(
                111,
                "accounts weight or threshold not valid".to_owned(),
            );
        }

        // check the recursion depth
        if payload
            .addr_with_weight
            .iter()
            .map(|s| self._is_recursion_depth_overflow(&s.address, 0))
            .any(|res| res)
        {
            return ServiceResponse::<GenerateMultiSigAccountResponse>::from_error(
                116,
                "above max recursion depth".to_owned(),
            );
        }

        if let Ok(address) = Address::from_hash(Hash::digest(
            ctx.get_tx_hash().expect("Can not get tx hash").as_bytes(),
        )) {
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

            let owner = if payload.autonomy == true {
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
            ServiceResponse::<GenerateMultiSigAccountResponse>::from_error(
                112,
                "generate address from tx_hash failed".to_owned(),
            )
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
            ServiceResponse::<GetMultiSigAccountResponse>::from_error(
                113,
                "account not existed".to_owned(),
            )
        }
    }

    #[cycles(21_000)]
    #[read]
    pub fn verify_signature(
        &self,
        ctx: ServiceContext,
        payload: SignedTransaction,
    ) -> ServiceResponse<()> {
        let pubkeys = if let Ok(pubkeys_bytes) =
            catch_unwind(|| rlp::decode_list::<Vec<u8>>(&payload.pubkey.to_vec()))
        {
            pubkeys_bytes
        } else {
            return ServiceResponse::<()>::from_error(122, "decode pubkey failed".to_owned());
        };

        let sigs = if let Ok(sigs_bytes) =
            catch_unwind(|| rlp::decode_list::<Vec<u8>>(&payload.signature.to_vec()))
        {
            sigs_bytes
        } else {
            return ServiceResponse::<()>::from_error(122, "decode signatures failed".to_owned());
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
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            // check if account contains itself
            if payload
                .addr_with_weight
                .iter()
                .map(|a| a.address.clone())
                .any(|addr| addr == payload.account_address)
            {
                return ServiceResponse::<()>::from_error(
                    115,
                    "account can not contain itself".to_owned(),
                );
            }

            // check sum of weight
            if payload.addr_with_weight.is_empty()
                || payload.addr_with_weight.len() > MAX_PERMISSION_ACCOUNTS as usize
            {
                return ServiceResponse::<()>::from_error(
                    110,
                    "accounts length must be [1,16]".to_owned(),
                );
            }

            let weight_sum = payload
                .addr_with_weight
                .iter()
                .map(|item| item.weight as u32)
                .sum::<u32>();

            // check if sum of the weights is above threshold
            if payload.threshold == 0 || weight_sum < payload.threshold {
                return ServiceResponse::<()>::from_error(
                    111,
                    "accounts weight or threshold not valid".to_owned(),
                );
            }

            // check the recursion depth
            if payload
                .addr_with_weight
                .iter()
                .map(|s| self._is_recursion_depth_overflow(&s.address, 0))
                .any(|res| res)
            {
                return ServiceResponse::<()>::from_error(
                    116,
                    "above max recursion depth".to_owned(),
                );
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

        ServiceResponse::<()>::from_error(113, "account not existed".to_owned())
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
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            // check new owner's recursion depth
            if self._is_recursion_depth_overflow(&payload.new_owner, 0) {
                return ServiceResponse::<()>::from_error(
                    116,
                    "new owner above max recursion depth".to_owned(),
                );
            }

            permission.set_owner(payload.new_owner);
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceResponse::<()>::from_error(113, "account not existed".to_owned())
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
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            permission.set_memo(payload.new_memo);
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceResponse::<()>::from_error(113, "account not existed".to_owned())
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
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            // check whether reach the max count
            if permission.accounts.len() == MAX_PERMISSION_ACCOUNTS as usize {
                return ServiceResponse::<()>::from_error(
                    119,
                    "the account count reach max value".to_owned(),
                );
            }

            // check whether the new account above max recursion depth
            if self._is_recursion_depth_overflow(&payload.new_account.address, 1) {
                return ServiceResponse::<()>::from_error(
                    116,
                    "new account above max recursion depth".to_owned(),
                );
            }

            permission.add_account(payload.new_account.clone());
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);

            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceResponse::<()>::from_error(113, "account not existed".to_owned())
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
                return ServiceResponse::<Account>::from_error(118, "invalid owner".to_owned());
            }

            match permission.remove_account(&payload.account_address) {
                RemoveAccountResult::Success(ret) => {
                    self.sdk
                        .set_account_value(&payload.multi_sig_address, 0u8, permission);
                    return ServiceResponse::<Account>::from_succeed(ret);
                }
                RemoveAccountResult::BelowThreshold => {
                    return ServiceResponse::<Account>::from_error(
                        121,
                        "the sum of weight will below threshold".to_owned(),
                    );
                }
                _ => (),
            }
        }
        ServiceResponse::<Account>::from_error(113, "account not existed".to_owned())
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
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            match permission.set_account_weight(&payload.account_address, payload.new_weight) {
                SetWeightResult::Success => {
                    self.sdk
                        .set_account_value(&payload.multi_sig_address, 0u8, permission);
                    return ServiceResponse::<()>::from_succeed(());
                }
                SetWeightResult::InvalidNewWeight => {
                    return ServiceResponse::<()>::from_error(
                        121,
                        "the sum of weight will below threshold".to_owned(),
                    );
                }
                _ => (),
            }
        }
        ServiceResponse::<()>::from_error(113, "account not existed".to_owned())
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
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            // check new threshold
            if permission
                .accounts
                .iter()
                .map(|account| account.weight as u32)
                .sum::<u32>()
                < payload.new_threshold
            {
                return ServiceResponse::<()>::from_error(
                    121,
                    "new threshold larger the sum of the weights".to_owned(),
                );
            }

            permission.set_threshold(payload.new_threshold);
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceResponse::<()>::from_error(113, "account not existed".to_owned())
        }
    }

    fn _inner_verify_signature(&self, payload: VerifySignaturePayload) -> ServiceResponse<()> {
        let pubkeys = payload.pubkeys.clone();
        let signatures = payload.signatures.clone();

        if pubkeys.len() != signatures.len() {
            return ServiceResponse::<()>::from_error(
                114,
                "pubkkeys len is not equal to signatures len".to_owned(),
            );
        }

        if pubkeys.len() == 1 {
            if let Ok(addr) = Address::from_pubkey_bytes(pubkeys[0].clone()) {
                if addr == payload.sender {
                    return self._verify_single_signature(
                        &payload.tx_hash,
                        &signatures[0],
                        &pubkeys[0],
                    );
                }
            } else {
                return ServiceResponse::<()>::from_error(123, "invalid public key".to_owned());
            }
        }

        self._verify_multi_signature(
            &payload.tx_hash,
            &Witness::new(pubkeys, signatures).to_addr_map(),
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
            return ServiceResponse::<()>::from_error(116, "above max recursion depth".to_owned());
        }

        let mut weight_acc = 0u32;

        let permission = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(sender, &0u8);
        if permission.is_none() {
            return ServiceResponse::<()>::from_error(113, "account not existed".to_owned());
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

        ServiceResponse::<()>::from_error(117, "multi signature not verified".to_owned())
    }

    fn _verify_single_signature(
        &self,
        tx_hash: &Hash,
        sig: &Bytes,
        pubkey: &Bytes,
    ) -> ServiceResponse<()> {
        if Secp256k1::verify_signature(tx_hash.as_bytes().as_ref(), sig.as_ref(), pubkey.as_ref())
            .is_ok()
        {
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceResponse::<()>::from_error(117, "signature verified failed".to_owned())
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
