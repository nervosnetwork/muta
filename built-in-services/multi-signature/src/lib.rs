#[cfg(test)]
mod tests;
pub mod types;

use binding_macro::{cycles, service};

use common_crypto::{Crypto, Secp256k1};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{Address, Bytes, Hash, Hex, PubkeyWithSender, ServiceContext};

use crate::types::{
    AddAccountPayload, ChangeOwnerPayload, GenerateMultiSigAccountPayload,
    GenerateMultiSigAccountResponse, GetMultiSigAccountPayload, GetMultiSigAccountResponse,
    MultiSigAccount, MultiSigPermission, RemoveAccountPayload, SetAccountWeightPayload,
    SetThresholdPayload, VerifyMultiSigPayload, Witness, MAX_PERMISSION_ACCOUNTS,
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

        let mut weight_sum = 0;
        let accounts = payload
            .accounts
            .iter()
            .map(|account| {
                weight_sum += account.weight as u32;
                MultiSigAccount {
                    address: account.address.clone(),
                    weight:  account.weight,
                }
            })
            .collect::<Vec<_>>();

        if payload.threshold == 0 || weight_sum < payload.threshold {
            return ServiceResponse::<GenerateMultiSigAccountResponse>::from_error(
                110,
                "accounts weight or threshold not valid".to_owned(),
            );
        }

        if let Ok(address) = Address::from_hash(Hash::digest(
            ctx.get_tx_hash().expect("Can not get tx hash").as_bytes(),
        )) {
            let permission = MultiSigPermission {
                accounts,
                owner: payload.owner,
                threshold: payload.threshold,
            };
            self.sdk.set_account_value(&address, 0u8, permission);

            ServiceResponse::<GenerateMultiSigAccountResponse>::from_succeed(
                GenerateMultiSigAccountResponse { address },
            )
        } else {
            ServiceResponse::<GenerateMultiSigAccountResponse>::from_error(
                111,
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
                110,
                "account not existed".to_owned(),
            )
        }
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<()>::from_error(121, "invalid owner".to_owned());
            }

            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<()>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
                );
            }

            permission.set_owner(payload.new_owner.clone());
            self.sdk
                .set_account_value(&payload.multi_sig_address, 0u8, permission);
            ServiceResponse::<()>::from_succeed(())
        } else {
            ServiceResponse::<()>::from_error(110, "account not existed".to_owned())
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<()>::from_error(121, "invalid owner".to_owned());
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
            ServiceResponse::<()>::from_error(110, "account not existed".to_owned())
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<MultiSigAccount>::from_error(
                    121,
                    "invalid owner".to_owned(),
                );
            }

            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<MultiSigAccount>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
                );
            }

            if let Some(ret) = permission.remove_account(&payload.account_address) {
                self.sdk
                    .set_account_value(&payload.multi_sig_address, 0u8, permission);
                return ServiceResponse::<MultiSigAccount>::from_succeed(ret);
            }
        }
        ServiceResponse::<MultiSigAccount>::from_error(110, "account not existed".to_owned())
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<()>::from_error(121, "invalid owner".to_owned());
            }

            if self.verify_signature(ctx, payload.witness).is_error() {
                return ServiceResponse::<()>::from_error(
                    120,
                    "owner signature verified failed".to_owned(),
                );
            }

            if permission
                .set_account_weight(&payload.account_address, payload.new_weight)
                .is_some()
            {
                self.sdk
                    .set_account_value(&payload.multi_sig_address, 0u8, permission);
                return ServiceResponse::<()>::from_succeed(());
            }
        }
        ServiceResponse::<()>::from_error(110, "account not existed".to_owned())
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
            if permission.owner != payload.witness.sender {
                return ServiceResponse::<()>::from_error(121, "invalid owner".to_owned());
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
            ServiceResponse::<()>::from_error(110, "account not existed".to_owned())
        }
    }

    #[cycles(100_00)]
    #[read]
    fn verify_signature(
        &self,
        ctx: ServiceContext,
        payload: VerifyMultiSigPayload,
    ) -> ServiceResponse<()> {
        let mut recursion_depth = 0u8;
        let mut weight_acc = 0u32;
        self._verify_multi_signature(
            &ctx.get_tx_hash().unwrap(),
            &Witness::new(payload.pubkeys, payload.signatures),
            &payload.sender,
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
        if witness.signatures.len() != witness.pubkeys.len() {
            return ServiceResponse::<()>::from_error(
                116,
                "len of signatures and pubkeys must be equal".to_owned(),
            );
        }

        if witness.signatures.len() > MAX_PERMISSION_ACCOUNTS as usize {
            return ServiceResponse::<()>::from_error(
                117,
                "len of signatures must be [1,16]".to_owned(),
            );
        }

        let permission = self
            .sdk
            .get_account_value::<_, MultiSigPermission>(sender, &0u8);
        if permission.is_none() {
            return ServiceResponse::<()>::from_error(117, "account not existed".to_owned());
        }

        let permission = permission.unwrap();

        for (sig, pk) in witness.signatures.iter().zip(witness.pubkeys.iter()) {
            let temp = rlp::decode::<PubkeyWithSender>(pk);
            if temp.is_err() {
                continue;
            }
            let pk_with_sender = temp.unwrap();

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
                        119,
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
                    add_weight_by_address(&permission, pk_with_sender.pubkey.clone(), weight_acc);
                }
            }

            if *weight_acc >= permission.threshold {
                return ServiceResponse::<()>::from_succeed(());
            }
        }

        ServiceResponse::<()>::from_error(111, "multi signature not verified".to_owned())
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
            ServiceResponse::<()>::from_error(110, "signature not verified".to_owned())
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

fn decode_multi_sigs_and_pubkeys(
    sig: &Bytes,
    pk_with_sender: &PubkeyWithSender,
) -> (Vec<Bytes>, Vec<Bytes>, Address) {
    let pks = rlp::decode_list::<Vec<u8>>(pk_with_sender.pubkey.as_ref())
        .into_iter()
        .map(Bytes::from)
        .collect::<Vec<_>>();
    let sigs = rlp::decode_list::<Vec<u8>>(sig)
        .into_iter()
        .map(Bytes::from)
        .collect::<Vec<_>>();
    let sub_sender = pk_with_sender.sender.clone().unwrap();

    (pks, sigs, sub_sender)
}
