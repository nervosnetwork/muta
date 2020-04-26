use crate::types::{
    Account, GenerateAccountPayload, GenerateAccountResponse, GetAccountPayload, PayloadAccount,
    Permission, VerifyPayload, VerifyResponse, ACCOUNT_TYPE_PUBLIC_KEY, MAX_PERMISSION_ACCOUNTS,
};
use binding_macro::{cycles, service};
use bytes::Bytes;
use hasher::{Hasher, HasherKeccak};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{Address, Hash, ServiceContext};

#[cfg(test)]
mod tests;
pub mod types;

pub struct AccountService<SDK> {
    sdk: SDK,
}

#[service]
impl<SDK: ServiceSDK> AccountService<SDK> {
    pub fn new(mut sdk: SDK) -> Self {
        Self { sdk }
    }

    #[cycles(100_00)]
    #[read]
    fn verify(
        &self,
        ctx: ServiceContext,
        payload: VerifyPayload,
    ) -> ServiceResponse<VerifyResponse> {
        ServiceResponse::<VerifyResponse>::from_error(
            110,
            "accounts length must be [1,16]".to_owned(),
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

        let mut accounts = Vec::<PayloadAccount>::new();
        for item in &permission.accounts {
            accounts.push(PayloadAccount {
                address: item.address.clone(),
                weight:  item.weight,
            });
        }

        let response = GenerateAccountResponse {
            accounts,
            threshold: permission.threshold,
            address: payload.user.clone(),
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
        if payload.accounts.len() == 0 || payload.accounts.len() > MAX_PERMISSION_ACCOUNTS as usize
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

        let keccak = HasherKeccak::new();
        let addr_hash = Hash::from_bytes(Bytes::from(keccak.digest(&tx_hash.as_bytes())));
        if addr_hash.is_err() {
            return ServiceResponse::<GenerateAccountResponse>::from_error(
                111,
                "generate addr_hash from tx_hash failed".to_owned(),
            );
        }

        let addr = Address::from_hash(addr_hash.unwrap());
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

        let response = GenerateAccountResponse {
            address:   address.clone(),
            accounts:  payload.accounts,
            threshold: payload.threshold,
        };

        ServiceResponse::<GenerateAccountResponse>::from_succeed(response)
    }
}
