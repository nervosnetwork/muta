#[cfg(test)]
mod tests;
pub mod types;

use binding_macro::{cycles, service};

use common_crypto::{Crypto, Secp256k1};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{Address, Bytes, Hash, PubkeyWithSender, ServiceContext};

use crate::types::{
    AddAccountPayload, ChangeMemoPayload, ChangeOwnerPayload, GenerateMultiSigAccountPayload,
    GenerateMultiSigAccountResponse, GetMultiSigAccountPayload, GetMultiSigAccountResponse,
    MultiSigAccount, MultiSigPermission, RemoveAccountPayload, RemoveAccountResult,
    SetAccountWeightPayload, SetThresholdPayload, SetWeightResult, VerifySignaturePayload, Witness,
    MAX_PERMISSION_ACCOUNTS,
};

const MAX_MULTI_SIGNATURE_RECURSION_DEPTH: u8 = 16;

pub struct MultiSignatureService<SDK> {
    sdk: SDK,
}

#[service]
impl<SDK: ServiceSDK> MultiSignatureService<SDK> {
    pub fn new(sdk: SDK) -> Self {
        MultiSignatureService { sdk }
    }

    #[cycles(210_00)]
    #[write]
    fn generate_account(
        &mut self,
        ctx: ServiceContext,
        payload: GenerateMultiSigAccountPayload,
    ) -> ServiceResponse<GenerateMultiSigAccountResponse> {
        if payload.accounts.is_empty() || payload.accounts.len() > MAX_PERMISSION_ACCOUNTS as usize
        {
            return ServiceResponse::<GenerateMultiSigAccountResponse>::from_error(
                110,
                "accounts length must be [1,16]".to_owned(),
            );
        }

        let weight_sum = payload
            .accounts
            .iter()
            .map(|account| account.weight as u32)
            .sum::<u32>();

        if payload.threshold == 0 || weight_sum < payload.threshold {
            return ServiceResponse::<GenerateMultiSigAccountResponse>::from_error(
                111,
                "accounts weight or threshold not valid".to_owned(),
            );
        }

        if let Ok(address) = Address::from_hash(Hash::digest(
            ctx.get_tx_hash().expect("Can not get tx hash").as_bytes(),
        )) {
            let permission = MultiSigPermission {
                accounts:  payload.accounts,
                owner:     payload.owner,
                threshold: payload.threshold,
                memo:      payload.memo,
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
        ctx: ServiceContext,
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

    #[cycles(100_00)]
    #[read]
    fn verify_signature(
        &self,
        ctx: ServiceContext,
        payload: VerifySignaturePayload,
    ) -> ServiceResponse<()> {
        let tmp = rlp::decode::<PubkeyWithSender>(payload.pubkeys.as_ref());
        if tmp.is_err() {
            return ServiceResponse::<()>::from_error(
                114,
                "decode pubkey_with_sender error".to_owned(),
            );
        }
        let pk_with_sender = tmp.unwrap();

        if pk_with_sender.sender.is_none() {
            return self._verify_single_signature(
                &ctx.get_tx_hash().unwrap(),
                &payload.signatures,
                &pk_with_sender.pubkey,
            );
        }

        let wit = Witness::new(
            rlp::decode_list::<PubkeyWithSender>(pk_with_sender.pubkey.as_ref()),
            rlp::decode_list::<Vec<u8>>(payload.signatures.as_ref()),
        );

        let mut recursion_depth = 0u8;
        let mut weight_acc = 0u32;
        self._verify_multi_signature(
            &ctx.get_tx_hash().unwrap(),
            &wit,
            &pk_with_sender.sender.unwrap(),
            &mut weight_acc,
            &mut recursion_depth,
        )
    }

    fn _verify_multi_signature(
        &self,
        tx_hash: &Hash,
        witness: &Witness,
        sender: &Address,
        weight_acc: &mut u32,
        recursion_depth: &mut u8,
    ) -> ServiceResponse<()> {
        if witness.signatures.len() > MAX_PERMISSION_ACCOUNTS as usize {
            return ServiceResponse::<()>::from_error(
                115,
                "len of signatures must be [1,16]".to_owned(),
            );
        }

        let permission = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(sender, &0u8);
        if permission.is_none() {
            return ServiceResponse::<()>::from_error(113, "account not existed".to_owned());
        }

        let permission = permission.unwrap();

        for (sig, pk_with_sender) in witness.signatures.iter().zip(witness.pubkeys.iter()) {
            if pk_with_sender.sender.is_none() {
                if !self
                    ._verify_single_signature(tx_hash, sig, &pk_with_sender.pubkey)
                    .is_error()
                {
                    add_weight_by_address(&permission, pk_with_sender.pubkey.clone(), weight_acc);
                }
            } else {
                *recursion_depth += 1;
                if *recursion_depth >= MAX_MULTI_SIGNATURE_RECURSION_DEPTH {
                    return ServiceResponse::<()>::from_error(
                        116,
                        "the recursion of multiple signatures should be less than 16".to_owned(),
                    );
                }

                let (pks, sigs, sub_sender) = decode_multi_sigs_and_pubkeys(sig, &pk_with_sender);
                let mut sub_weight_acc = 0u32;
                if !self
                    ._verify_multi_signature(
                        tx_hash,
                        &Witness::new(pks, sigs),
                        &sub_sender,
                        &mut sub_weight_acc,
                        recursion_depth,
                    )
                    .is_error()
                {
                    add_weight_by_sender(&permission, &sub_sender, weight_acc);
                }
            }

            if *weight_acc >= permission.threshold {
                return ServiceResponse::<()>::from_succeed(());
            }
        }

        ServiceResponse::<()>::from_error(117, "multi signature not verified".to_owned())
    }

    #[cycles(100_00)]
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
            if Some(permission.owner.clone()) != payload.witness.get_sender() {
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<()>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
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

    #[cycles(100_00)]
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
            if Some(permission.owner.clone()) != payload.witness.get_sender() {
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

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
            if Some(permission.owner.clone()) != payload.witness.get_sender() {
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

            if permission.accounts.len() == MAX_PERMISSION_ACCOUNTS as usize {
                return ServiceResponse::<()>::from_error(
                    119,
                    "the account count reach max value".to_owned(),
                );
            }

            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<()>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
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
    ) -> ServiceResponse<MultiSigAccount> {
        if let Some(mut permission) = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(&payload.multi_sig_address, &0u8)
        {
            if Some(permission.owner.clone()) != payload.witness.get_sender() {
                return ServiceResponse::<MultiSigAccount>::from_error(
                    118,
                    "invalid owner".to_owned(),
                );
            }

            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<MultiSigAccount>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
                );
            }

            match permission.remove_account(&payload.account_address) {
                RemoveAccountResult::Success(ret) => {
                    self.sdk
                        .set_account_value(&payload.multi_sig_address, 0u8, permission);
                    return ServiceResponse::<MultiSigAccount>::from_succeed(ret);
                }
                RemoveAccountResult::BelowThreshold => {
                    return ServiceResponse::<MultiSigAccount>::from_error(
                        121,
                        "the sum of weight will below threshold after remove the account"
                            .to_owned(),
                    );
                }
                _ => (),
            }
        }
        ServiceResponse::<MultiSigAccount>::from_error(113, "account not existed".to_owned())
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
            if Some(permission.owner.clone()) != payload.witness.get_sender() {
                return ServiceResponse::<()>::from_error(118, "invalid owner".to_owned());
            }

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
                        122,
                        "new weight is invalid".to_owned(),
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
            if Some(permission.owner.clone()) != payload.witness.get_sender() {
                return ServiceResponse::<()>::from_error(121, "invalid owner".to_owned());
            }

            if permission
                .accounts
                .iter()
                .map(|account| account.weight as u32)
                .sum::<u32>()
                < payload.new_threshold
            {
                return ServiceResponse::<()>::from_error(
                    122,
                    "new threshold larger the sum of the weights".to_owned(),
                );
            }

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
            ServiceResponse::<()>::from_error(117, "signature not verified".to_owned())
        }
    }
}

fn add_weight_by_address(permission: &MultiSigPermission, pubkey: Bytes, weight_acc: &mut u32) {
    let _ = Address::from_pubkey_bytes(pubkey).and_then(|addr| {
        *weight_acc += permission
            .get_account(&addr)
            .map_or_else(|| 0u32, |account| account.weight as u32);
        Ok(())
    });
}

fn add_weight_by_sender(permission: &MultiSigPermission, sender: &Address, weight_acc: &mut u32) {
    *weight_acc += permission
        .get_account(sender)
        .map_or_else(|| 0u32, |account| account.weight as u32);
}

fn decode_multi_sigs_and_pubkeys(
    sig: &Bytes,
    pk_with_sender: &PubkeyWithSender,
) -> (Vec<PubkeyWithSender>, Vec<Vec<u8>>, Address) {
    let pks = rlp::decode_list::<PubkeyWithSender>(pk_with_sender.pubkey.as_ref());
    let sigs = rlp::decode_list::<Vec<u8>>(sig);
    let sub_sender = pk_with_sender.sender.clone().unwrap();
    (pks, sigs, sub_sender)
}
