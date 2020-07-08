#![allow(clippy::mutable_key_type)]

#[cfg(test)]
mod tests;
pub mod types;

use std::collections::BTreeMap;

use binding_macro::{cycles, genesis, service};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK, StoreMap};
use protocol::types::{Address, Bytes, Hash, ServiceContext};

use crate::types::{
    ApproveEvent, ApprovePayload, Asset, AssetBalance, CreateAssetPayload, GetAllowancePayload,
    GetAllowanceResponse, GetAssetPayload, GetBalancePayload, GetBalanceResponse,
    InitGenesisPayload, TransferEvent, TransferFromEvent, TransferFromPayload, TransferPayload,
};

pub struct AssetService<SDK> {
    sdk:    SDK,
    assets: Box<dyn StoreMap<Hash, Asset>>,
}

#[service]
impl<SDK: ServiceSDK> AssetService<SDK> {
    pub fn new(mut sdk: SDK) -> Self {
        let assets: Box<dyn StoreMap<Hash, Asset>> = sdk.alloc_or_recover_map("assets");

        Self { sdk, assets }
    }

    #[genesis]
    fn init_genesis(&mut self, payload: InitGenesisPayload) {
        let asset = Asset {
            id:     payload.id,
            name:   payload.name,
            symbol: payload.symbol,
            supply: payload.supply,
            issuer: payload.issuer.clone(),
        };

        self.assets.insert(asset.id.clone(), asset.clone());

        let asset_balance = AssetBalance {
            value:     payload.supply,
            allowance: BTreeMap::new(),
        };

        self.sdk
            .set_account_value(&asset.issuer, asset.id, asset_balance)
    }

    #[cycles(100_00)]
    #[read]
    fn get_asset(&self, ctx: ServiceContext, payload: GetAssetPayload) -> ServiceResponse<Asset> {
        if let Some(asset) = self.assets.get(&payload.id) {
            ServiceResponse::<Asset>::from_succeed(asset)
        } else {
            ServiceResponse::<Asset>::from_error(101, "asset id not existed".to_owned())
        }
    }

    #[cycles(100_00)]
    #[read]
    fn get_balance(
        &self,
        ctx: ServiceContext,
        payload: GetBalancePayload,
    ) -> ServiceResponse<GetBalanceResponse> {
        if !self.assets.contains(&payload.asset_id) {
            return ServiceResponse::<GetBalanceResponse>::from_error(
                101,
                "asset id not existed".to_owned(),
            );
        }

        let asset_balance = self
            .sdk
            .get_account_value(&payload.user, &payload.asset_id)
            .unwrap_or(AssetBalance {
                value:     0,
                allowance: BTreeMap::new(),
            });

        let res = GetBalanceResponse {
            asset_id: payload.asset_id,
            user:     payload.user,
            balance:  asset_balance.value,
        };

        ServiceResponse::<GetBalanceResponse>::from_succeed(res)
    }

    #[cycles(100_00)]
    #[read]
    fn get_allowance(
        &self,
        ctx: ServiceContext,
        payload: GetAllowancePayload,
    ) -> ServiceResponse<GetAllowanceResponse> {
        if !self.assets.contains(&payload.asset_id) {
            return ServiceResponse::<GetAllowanceResponse>::from_error(
                101,
                "asset id not existed".to_owned(),
            );
        }

        let opt_asset_balance: Option<AssetBalance> = self
            .sdk
            .get_account_value(&payload.grantor, &payload.asset_id);

        if let Some(v) = opt_asset_balance {
            let allowance = v.allowance.get(&payload.grantee).unwrap_or(&0);

            let res = GetAllowanceResponse {
                asset_id: payload.asset_id,
                grantor:  payload.grantor,
                grantee:  payload.grantee,
                value:    *allowance,
            };
            ServiceResponse::<GetAllowanceResponse>::from_succeed(res)
        } else {
            let res = GetAllowanceResponse {
                asset_id: payload.asset_id,
                grantor:  payload.grantor,
                grantee:  payload.grantee,
                value:    0,
            };
            ServiceResponse::<GetAllowanceResponse>::from_succeed(res)
        }
    }

    #[cycles(210_00)]
    #[write]
    fn create_asset(
        &mut self,
        ctx: ServiceContext,
        payload: CreateAssetPayload,
    ) -> ServiceResponse<Asset> {
        let caller = ctx.get_caller();
        let payload_res = serde_json::to_string(&payload);

        if let Err(e) = payload_res {
            return ServiceResponse::<Asset>::from_error(103, format!("{:?}", e));
        }
        let payload_str = payload_res.unwrap();

        let id = Hash::digest(Bytes::from(payload_str + &caller.as_hex()));

        if self.assets.contains(&id) {
            return ServiceResponse::<Asset>::from_error(102, "asset id existed".to_owned());
        }
        let asset = Asset {
            id:     id.clone(),
            name:   payload.name,
            symbol: payload.symbol,
            supply: payload.supply,
            issuer: caller,
        };
        self.assets.insert(id, asset.clone());

        let asset_balance = AssetBalance {
            value:     payload.supply,
            allowance: BTreeMap::new(),
        };

        self.sdk
            .set_account_value(&asset.issuer, asset.id.clone(), asset_balance);

        let event_res = serde_json::to_string(&asset);

        if let Err(e) = event_res {
            return ServiceResponse::<Asset>::from_error(103, format!("{:?}", e));
        }
        let event_str = event_res.unwrap();
        ctx.emit_event("Create".to_owned(), event_str);

        ServiceResponse::<Asset>::from_succeed(asset)
    }

    #[cycles(210_00)]
    #[write]
    fn transfer(&mut self, ctx: ServiceContext, payload: TransferPayload) -> ServiceResponse<()> {
        let caller = ctx.get_caller();
        let asset_id = payload.asset_id.clone();
        let value = payload.value;
        let to = payload.to;

        if !self.assets.contains(&payload.asset_id) {
            return ServiceResponse::<()>::from_error(101, "asset id not existed".to_owned());
        }

        if let Err(e) = self._transfer(caller.clone(), to.clone(), asset_id.clone(), value) {
            return ServiceResponse::<()>::from_error(106, format!("{:?}", e));
        };

        let event = TransferEvent {
            asset_id,
            from: caller,
            to,
            value,
        };
        let event_res = serde_json::to_string(&event);

        if let Err(e) = event_res {
            return ServiceResponse::<()>::from_error(103, format!("{:?}", e));
        };
        let event_str = event_res.unwrap();
        ctx.emit_event("Transfer".to_owned(), event_str);

        ServiceResponse::<()>::from_succeed(())
    }

    #[cycles(210_00)]
    #[write]
    fn approve(&mut self, ctx: ServiceContext, payload: ApprovePayload) -> ServiceResponse<()> {
        let caller = ctx.get_caller();
        let asset_id = payload.asset_id.clone();
        let value = payload.value;
        let to = payload.to;

        if caller == to {
            return ServiceResponse::<()>::from_error(104, "cann't approve to yourself".to_owned());
        }

        if !self.assets.contains(&payload.asset_id) {
            return ServiceResponse::<()>::from_error(101, "asset id not existed".to_owned());
        }

        let mut caller_asset_balance: AssetBalance = self
            .sdk
            .get_account_value(&caller, &asset_id)
            .unwrap_or(AssetBalance {
                value:     0,
                allowance: BTreeMap::new(),
            });
        caller_asset_balance
            .allowance
            .entry(to.clone())
            .and_modify(|e| *e = value)
            .or_insert(value);

        self.sdk
            .set_account_value(&caller, asset_id.clone(), caller_asset_balance);

        let event = ApproveEvent {
            asset_id,
            grantor: caller,
            grantee: to,
            value,
        };
        let event_res = serde_json::to_string(&event);

        if let Err(e) = event_res {
            return ServiceResponse::<()>::from_error(103, format!("{:?}", e));
        };
        let event_str = event_res.unwrap();
        ctx.emit_event("Approve".to_owned(), event_str);

        ServiceResponse::<()>::from_succeed(())
    }

    #[cycles(210_00)]
    #[write]
    fn transfer_from(
        &mut self,
        ctx: ServiceContext,
        payload: TransferFromPayload,
    ) -> ServiceResponse<()> {
        let caller = ctx.get_caller();
        let sender = payload.sender;
        let recipient = payload.recipient;
        let asset_id = payload.asset_id;
        let value = payload.value;

        if !self.assets.contains(&asset_id) {
            return ServiceResponse::<()>::from_error(101, "asset id not existed".to_owned());
        }

        let mut sender_asset_balance: AssetBalance = self
            .sdk
            .get_account_value(&sender, &asset_id)
            .unwrap_or(AssetBalance {
                value:     0,
                allowance: BTreeMap::new(),
            });
        let sender_allowance = sender_asset_balance
            .allowance
            .entry(caller.clone())
            .or_insert(0);
        if *sender_allowance < value {
            return ServiceResponse::<()>::from_error(105, "insufficient balance".to_owned());
        }
        let after_sender_allowance = *sender_allowance - value;
        sender_asset_balance
            .allowance
            .entry(caller.clone())
            .and_modify(|e| *e = after_sender_allowance)
            .or_insert(after_sender_allowance);
        self.sdk
            .set_account_value(&sender, asset_id.clone(), sender_asset_balance);

        if let Err(e) = self._transfer(sender.clone(), recipient.clone(), asset_id.clone(), value) {
            return ServiceResponse::<()>::from_error(106, format!("{:?}", e));
        };

        let event = TransferFromEvent {
            asset_id,
            caller,
            sender,
            recipient,
            value,
        };
        let event_res = serde_json::to_string(&event);

        if let Err(e) = event_res {
            return ServiceResponse::<()>::from_error(103, format!("{:?}", e));
        };
        let event_str = event_res.unwrap();
        ctx.emit_event("TransferFrom".to_owned(), event_str);

        ServiceResponse::<()>::from_succeed(())
    }

    fn _transfer(
        &mut self,
        sender: Address,
        recipient: Address,
        asset_id: Hash,
        value: u64,
    ) -> Result<(), String> {
        if recipient == sender {
            return Err("cann't send value to yourself".to_owned());
        }

        let mut sender_asset_balance: AssetBalance = self
            .sdk
            .get_account_value(&sender, &asset_id)
            .unwrap_or(AssetBalance {
                value:     0,
                allowance: BTreeMap::new(),
            });
        let sender_balance = sender_asset_balance.value;

        if sender_balance < value {
            return Err("insufficient balance".to_owned());
        }

        let mut to_asset_balance: AssetBalance = self
            .sdk
            .get_account_value(&recipient, &asset_id)
            .unwrap_or(AssetBalance {
                value:     0,
                allowance: BTreeMap::new(),
            });

        let (v, overflow) = to_asset_balance.value.overflowing_add(value);
        if overflow {
            return Err("u64 overflow".to_owned());
        }
        to_asset_balance.value = v;

        self.sdk
            .set_account_value(&recipient, asset_id.clone(), to_asset_balance);

        let (v, overflow) = sender_balance.overflowing_sub(value);
        if overflow {
            return Err("u64 overflow".to_owned());
        }
        sender_asset_balance.value = v;
        self.sdk
            .set_account_value(&sender, asset_id, sender_asset_balance);

        Ok(())
    }
}
