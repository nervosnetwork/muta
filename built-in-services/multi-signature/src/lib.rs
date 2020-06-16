#![allow(clippy::suspicious_else_formatting)]

#[cfg(test)]
mod tests;
pub mod types;

use std::collections::HashMap;

use binding_macro::{cycles, genesis, service};

use common_crypto::{Crypto, Secp256k1};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{Address, Bytes, Hash, ServiceContext};

use crate::types::{
    Account, AddAccountPayload, ChangeMemoPayload, ChangeOwnerPayload,
    GenerateMultiSigAccountPayload, GenerateMultiSigAccountResponse, GetMultiSigAccountPayload,
    GetMultiSigAccountResponse, InitGenesisPayload, MultiSigPermission, RemoveAccountPayload,
    RemoveAccountResult, SetAccountWeightPayload, SetThresholdPayload, SetWeightResult,
    VerifySignaturePayload, Witness,
};

const MAX_MULTI_SIGNATURE_RECURSION_DEPTH: u8 = 8;
const MAX_PERMISSION_ACCOUNTS: u8 = 16;

pub struct MultiSignatureService<SDK> {
    sdk: SDK,
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

    #[cycles(210_00)]
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
        let depth = payload
            .addr_with_weight
            .iter()
            .map(|s| self._recursion_depth(&s.address))
            .max()
            .unwrap_or(0);
        if depth > MAX_MULTI_SIGNATURE_RECURSION_DEPTH {
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

            let permission = MultiSigPermission {
                accounts,
                owner: payload.owner,
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

    #[cycles(100_00)]
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

    #[cycles(210_00)]
    #[read]
    fn verify_signature(
        &self,
        ctx: ServiceContext,
        payload: VerifySignaturePayload,
    ) -> ServiceResponse<()> {
        let pubkeys = payload.pubkeys.clone();
        let signatures = payload.signatures.clone();

        if pubkeys.len() != signatures.len() {
            return ServiceResponse::<()>::from_error(
                114,
                "pubkkeys len is not equal to signatures len".to_owned(),
            );
        }

        if pubkeys.len() > MAX_PERMISSION_ACCOUNTS as usize {
            return ServiceResponse::<()>::from_error(
                115,
                "len of signatures must be [1,16]".to_owned(),
            );
        }

        if pubkeys.len() == 1 {
            if let Ok(addr) = Address::from_pubkey_bytes(pubkeys[0].clone()) {
                if addr == payload.sender {
                    return self._verify_single_signature(
                        &ctx.get_tx_hash().unwrap(),
                        &signatures[0],
                        &pubkeys[0],
                    );
                }
            } else {
                return ServiceResponse::<()>::from_error(123, "invalid public key".to_owned());
            }
        }

        let mut recursion_depth = 0u8;
        self._verify_multi_signature(
            &ctx.get_tx_hash().unwrap(),
            &Witness::new(pubkeys, signatures).to_addr_map(),
            &payload.sender,
            &mut recursion_depth,
        )
    }

    fn _verify_multi_signature(
        &self,
        tx_hash: &Hash,
        wit_map: &HashMap<Address, (Bytes, Bytes)>,
        sender: &Address,
        recursion_depth: &mut u8,
    ) -> ServiceResponse<()> {
        // check recursion depth
        *recursion_depth += 1;
        if *recursion_depth > MAX_MULTI_SIGNATURE_RECURSION_DEPTH {
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
            } else {
                if !self
                    ._verify_multi_signature(tx_hash, wit_map, &account.address, recursion_depth)
                    .is_error()
                {
                    weight_acc += account.weight as u32;
                }
            }

            if weight_acc >= permission.threshold {
                return ServiceResponse::<()>::from_succeed(());
            }
        }

        ServiceResponse::<()>::from_error(117, "multi signature not verified".to_owned())
    }

    #[cycles(210_00)]
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            // check owner signature
            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<()>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
                );
            }

            // check new owner's recursion depth
            if self._recursion_depth(&payload.new_owner) > MAX_MULTI_SIGNATURE_RECURSION_DEPTH {
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

    #[cycles(210_00)]
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            // check owner signature
            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<()>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
                );
            }

            permission.set_memo(payload.new_memo);
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceResponse::<()>::from_error(113, "account not existed".to_owned())
        }
    }

    #[cycles(210_00)]
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            // check whether reach the max count
            if permission.accounts.len() == MAX_PERMISSION_ACCOUNTS as usize {
                return ServiceResponse::<()>::from_error(
                    119,
                    "the account count reach max value".to_owned(),
                );
            }

            // check owner signature
            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<()>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
                );
            }

            // check whether the new account above max recursion depth
            if self._recursion_depth(&payload.new_account.address)
                > MAX_MULTI_SIGNATURE_RECURSION_DEPTH - 1
            {
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

    #[cycles(210_00)]
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<Account>::from_error(118, "invalid owner".to_owned());
            }

            // check owner signature
            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<Account>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
                );
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

    #[cycles(210_00)]
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            // check owner signature
            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<()>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
                );
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

    #[cycles(210_00)]
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<()>::from_error(121, "invalid owner".to_owned());
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

            // check owner signature
            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<()>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
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

    fn _recursion_depth(&self, address: &Address) -> u8 {
        if let Some(permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(address, &0u8)
        {
            let max = permission
                .accounts
                .iter()
                .filter(|account| account.is_multiple)
                .map(|account| self._recursion_depth(&account.address))
                .max()
                .unwrap_or(0);
            max + 1
        } else {
            0u8
        }
    }
}
