mod types;

use binding_macro::{cycles, genesis, service};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK, StoreArray};
use protocol::types::{Address, ServiceContext};

use crate::types::{
    AddVerifiedItemPayload, InitGenesisPayload, RemoveVerifiedItemPayload, SetAdminPayload,
    VerifiedItem,
};

const AUTHORIZATION_ADMIN_KEY: &str = "authotization_admin";

lazy_static::lazy_static! {
    static ref VERIFY_SIG: VerifiedItem = VerifiedItem {
        service_name: String::from("multi_signature"),
        method_name:  String::from("verify_signature"),
    };
}

pub struct AuthorizationService<SDK> {
    sdk:           SDK,
    verified_list: Box<dyn StoreArray<VerifiedItem>>,
}

#[service]
impl<SDK: ServiceSDK> AuthorizationService<SDK> {
    pub fn new(mut sdk: SDK) -> Self {
        let mut verified_list: Box<dyn StoreArray<VerifiedItem>> =
            sdk.alloc_or_recover_array("authotization");

        if verified_list.is_empty() {
            verified_list.push(VERIFY_SIG.clone());
        }

        Self { sdk, verified_list }
    }

    #[genesis]
    fn init_genesis(&mut self, payload: InitGenesisPayload) {
        self.sdk
            .set_value(AUTHORIZATION_ADMIN_KEY.to_string(), payload.admin);

        for item in payload.verified_items.into_iter() {
            self.verified_list.push(item);
        }
    }

    #[cycles(21_000)]
    #[read]
    fn check_authorization(&self, ctx: ServiceContext, payload: String) -> ServiceResponse<()> {
        for (_idx, item) in self.verified_list.iter() {
            let resp = self._do_verify(&ctx, &item.service_name, &item.method_name, &payload);
            if resp.is_error() {
                return ServiceResponse::<()>::from_error(
                    102,
                    format!(
                        "verify transaction {:?} error {:?}",
                        item.method_name, resp.error_message
                    ),
                );
            }
        }

        ServiceResponse::from_succeed(())
    }

    #[cycles(21_000)]
    #[write]
    fn add_verified_item(
        &mut self,
        ctx: ServiceContext,
        payload: AddVerifiedItemPayload,
    ) -> ServiceResponse<()> {
        if !self._is_admin(&ctx) {
            return ServiceResponse::<()>::from_error(103, "Invalid caller".to_owned());
        }

        let new_item = VerifiedItem::from(payload);
        if self._check_exist(&new_item).is_some() {
            return ServiceResponse::<()>::from_error(105, "Verified item exit".to_owned());
        }

        self.verified_list.push(new_item);
        ServiceResponse::from_succeed(())
    }

    #[cycles(21_000)]
    #[write]
    fn remove_verified_item(
        &mut self,
        ctx: ServiceContext,
        payload: RemoveVerifiedItemPayload,
    ) -> ServiceResponse<()> {
        if !self._is_admin(&ctx) {
            return ServiceResponse::<()>::from_error(103, "Invalid caller".to_owned());
        }

        let to_be_removed_item = VerifiedItem::from(payload);

        if to_be_removed_item == VERIFY_SIG.clone() {
            return ServiceResponse::<()>::from_error(
                105,
                "Can not remove verify signature".to_owned(),
            );
        }

        if let Some(index) = self._check_exist(&to_be_removed_item) {
            self.verified_list.remove(index);
            ServiceResponse::from_succeed(())
        } else {
            ServiceResponse::<()>::from_error(104, "Can not find item".to_owned())
        }
    }

    #[cycles(21_000)]
    #[write]
    fn set_admin(&mut self, ctx: ServiceContext, payload: SetAdminPayload) -> ServiceResponse<()> {
        if !self._is_admin(&ctx) {
            return ServiceResponse::<()>::from_error(103, "Invalid caller".to_owned());
        }

        self.sdk
            .set_value(AUTHORIZATION_ADMIN_KEY.to_string(), payload.new_admin);

        ServiceResponse::from_succeed(())
    }

    fn _do_verify(
        &self,
        ctx: &ServiceContext,
        service_name: &str,
        method_name: &str,
        payload_json: &str,
    ) -> ServiceResponse<String> {
        self.sdk
            .read(&ctx, None, service_name, method_name, &payload_json)
    }

    fn _is_admin(&self, ctx: &ServiceContext) -> bool {
        let admin: Address = self
            .sdk
            .get_value(&AUTHORIZATION_ADMIN_KEY.to_string())
            .expect("must have an admin");

        ctx.get_caller() == admin
    }

    fn _check_exist(&self, verified_item: &VerifiedItem) -> Option<u64> {
        let mut res = u64::MAX;
        for (idx, item) in self.verified_list.iter() {
            if &item == verified_item {
                res = idx as u64;
                break;
            }
        }

        if res == u64::MAX {
            None
        } else {
            Some(res)
        }
    }
}
