use crate::types::{
    Account, GenerateAccountPayload, GenerateAccountResponse, GetAccountPayload, Permission,
    VerifyPayload, VerifyResponse, WitnessAdapter, ACCOUNT_TYPE_MULTI_SIG, ACCOUNT_TYPE_PUBLIC_KEY,
    MAX_PERMISSION_ACCOUNTS,
};
use binding_macro::{cycles, service};
use bytes::Bytes;
use common_crypto::{Crypto, Secp256k1};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{Address, Hash, Hex, ServiceContext};

#[cfg(test)]
mod tests;
pub mod types;

pub struct AccountService<SDK> {
    sdk: SDK,
}

#[service]
impl<SDK: ServiceSDK> AccountService<SDK> {
    pub fn new(sdk: SDK) -> Self {
        Self { sdk }
    }

    #[cycles(100_00)]
    #[read]
    fn verify_signature(
        &self,
        ctx: ServiceContext,
        payload: VerifyPayload,
    ) -> ServiceResponse<VerifyResponse> {
        let wit_res: Result<WitnessAdapter, _> = serde_json::from_str(&payload.witness);
        if wit_res.is_err() {
            return ServiceResponse::<VerifyResponse>::from_error(
                113,
                "witness not valid".to_owned(),
            );
        }

        let wit = wit_res.unwrap();
        if wit.signature_type == ACCOUNT_TYPE_PUBLIC_KEY {
            if wit.signatures.len() != 1 || wit.pubkeys.len() != 1 {
                return ServiceResponse::<VerifyResponse>::from_error(
                    114,
                    "len of signatures or pubkeys must be 1".to_owned(),
                );
            }
            return verify_single_sig(&payload.tx_hash, &wit.signatures[0], &wit.pubkeys[0]);
        }

        // multi sig
        if wit.signature_type != ACCOUNT_TYPE_MULTI_SIG {
            return ServiceResponse::<VerifyResponse>::from_error(
                115,
                "signature_type not valid".to_owned(),
            );
        }

        if wit.signatures.len() != wit.pubkeys.len() {
            return ServiceResponse::<VerifyResponse>::from_error(
                116,
                "len of signatures and pubkeys must be equal".to_owned(),
            );
        }

        if wit.signatures.len() > MAX_PERMISSION_ACCOUNTS as usize {
            return ServiceResponse::<VerifyResponse>::from_error(
                117,
                "len of signatures must be [1,16]".to_owned(),
            );
        }

        let permission_res: Option<Permission> = self.sdk.get_account_value(&wit.sender, &0u8);

        if permission_res.is_none() {
            return ServiceResponse::<VerifyResponse>::from_error(
                117,
                "account not existed".to_owned(),
            );
        }

        let permission = permission_res.unwrap();
        let mut weight_sum = 0;
        let size = permission.accounts.len();
        let mut has_account = [false; MAX_PERMISSION_ACCOUNTS as usize];

        for i in 0..wit.signatures.len() {
            let res = verify_single_sig(&payload.tx_hash, &wit.signatures[i], &wit.pubkeys[i]);
            if res.is_error() {
                continue;
            }

            for (k, item) in permission.accounts.iter().enumerate().take(size) {
                if has_account[k] {
                    continue;
                }
                if item.address.eq(&res.succeed_data.address) {
                    has_account[k] = true;
                    weight_sum += item.weight;
                    break;
                }
            }

            if weight_sum >= permission.threshold {
                return ServiceResponse::<VerifyResponse>::from_succeed(VerifyResponse {
                    address: wit.sender,
                });
            }
        }

        ServiceResponse::<VerifyResponse>::from_error(
            111,
            "multi signature not verified".to_owned(),
        )
    }

    #[cycles(100_00)]
    #[read]
    fn get_account_from_address(
        &self,
        ctx: ServiceContext,
        payload: GetAccountPayload,
    ) -> ServiceResponse<GenerateAccountResponse> {
        let permission = self
            .sdk
            .get_account_value(&payload.user, &0u8)
            .unwrap_or(Permission {
                accounts:  Vec::<Account>::new(),
                threshold: 0,
            });

        if permission.threshold == 0 {
            return ServiceResponse::<GenerateAccountResponse>::from_error(
                110,
                "account not existed".to_owned(),
            );
        }

        let response = GenerateAccountResponse {
            address: payload.user,
        };

        ServiceResponse::<GenerateAccountResponse>::from_succeed(response)
    }

    #[cycles(210_00)]
    #[write]
    fn generate_account(
        &mut self,
        ctx: ServiceContext,
        payload: GenerateAccountPayload,
    ) -> ServiceResponse<GenerateAccountResponse> {
        if payload.accounts.is_empty() || payload.accounts.len() > MAX_PERMISSION_ACCOUNTS as usize
        {
            return ServiceResponse::<GenerateAccountResponse>::from_error(
                110,
                "accounts length must be [1,16]".to_owned(),
            );
        }

        let mut weight_all = 0;
        let mut accounts = Vec::<Account>::new();
        for item in &payload.accounts {
            weight_all += item.weight;
            accounts.push(Account {
                address:       item.address.clone(),
                account_type:  ACCOUNT_TYPE_PUBLIC_KEY,
                permission_id: 0,
                weight:        item.weight,
            });
        }

        if weight_all < payload.threshold || payload.threshold == 0 {
            return ServiceResponse::<GenerateAccountResponse>::from_error(
                110,
                "accounts weight or threshold not valid".to_owned(),
            );
        }

        let tx_hash = ctx.get_tx_hash().unwrap();

        let addr = Address::from_hash(Hash::digest(tx_hash.as_bytes()));
        if addr.is_err() {
            return ServiceResponse::<GenerateAccountResponse>::from_error(
                111,
                "generate address from tx_hash failed".to_owned(),
            );
        }

        let address = addr.unwrap();
        let permission = Permission {
            accounts,
            threshold: payload.threshold,
        };
        self.sdk.set_account_value(&address, 0u8, permission);

        let response = GenerateAccountResponse { address };
        ServiceResponse::<GenerateAccountResponse>::from_succeed(response)
    }
}

fn verify_single_sig(tx_hash: &Hash, sig: &Hex, pubkey: &Hex) -> ServiceResponse<VerifyResponse> {
    let data_hash = tx_hash.as_bytes();

    let data_sig = hex::decode(sig.as_string_trim0x());
    if data_sig.is_err() {
        return ServiceResponse::<VerifyResponse>::from_error(
            112,
            "signature not valid".to_owned(),
        );
    };

    let data_pk = hex::decode(pubkey.as_string_trim0x());
    if data_pk.is_err() {
        return ServiceResponse::<VerifyResponse>::from_error(
            113,
            "public key not valid".to_owned(),
        );
    };

    let pk = data_pk.unwrap();
    if Secp256k1::verify_signature(
        data_hash.as_ref(),
        data_sig.unwrap().as_slice(),
        pk.as_slice(),
    )
    .is_ok()
    {
        return ServiceResponse::<VerifyResponse>::from_succeed(VerifyResponse {
            address: Address::from_pubkey_bytes(Bytes::from(pk)).unwrap(),
        });
    }

    ServiceResponse::<VerifyResponse>::from_error(110, "signature not verified".to_owned())
}
