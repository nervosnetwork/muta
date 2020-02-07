#[cfg(test)]
mod tests;
pub mod types;

use std::collections::BTreeMap;

use bytes::Bytes;
use derive_more::{Display, From};

use binding_macro::{cycles, genesis, service, write};
use protocol::traits::{ExecutorParams, ServiceSDK, StoreMap};
use protocol::types::{Address, Hash, ServiceContext};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

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
    pub fn new(mut sdk: SDK) -> ProtocolResult<Self> {
        let assets: Box<dyn StoreMap<Hash, Asset>> = sdk.alloc_or_recover_map("assets")?;

        Ok(Self { sdk, assets })
    }

    #[genesis]
    fn init_genesis(&mut self, payload: InitGenesisPayload) -> ProtocolResult<()> {
        let asset = Asset {
            id:     payload.id,
            name:   payload.name,
            symbol: payload.symbol,
            supply: payload.supply,
            issuer: payload.issuer.clone(),
        };

        self.assets.insert(asset.id.clone(), asset.clone())?;

        let asset_balance = AssetBalance {
            value:     payload.supply,
            allowance: BTreeMap::new(),
        };

        self.sdk
            .set_account_value(&asset.issuer, asset.id, asset_balance)
    }

    #[cycles(100_00)]
    #[read]
    fn get_asset(&self, ctx: ServiceContext, payload: GetAssetPayload) -> ProtocolResult<Asset> {
        let asset = self.assets.get(&payload.id)?;
        Ok(asset)
    }

    #[cycles(100_00)]
    #[read]
    fn get_balance(
        &self,
        ctx: ServiceContext,
        payload: GetBalancePayload,
    ) -> ProtocolResult<GetBalanceResponse> {
        if !self.assets.contains(&payload.asset_id)? {
            return Err(ServiceError::NotFoundAsset {
                id: payload.asset_id,
            }
            .into());
        }

        let asset_balance = self
            .sdk
            .get_account_value(&payload.user, &payload.asset_id)?
            .unwrap_or(AssetBalance {
                value:     0,
                allowance: BTreeMap::new(),
            });

        Ok(GetBalanceResponse {
            asset_id: payload.asset_id,
            user:     payload.user,
            balance:  asset_balance.value,
        })
    }

    #[cycles(100_00)]
    #[read]
    fn get_allowance(
        &self,
        ctx: ServiceContext,
        payload: GetAllowancePayload,
    ) -> ProtocolResult<GetAllowanceResponse> {
        if !self.assets.contains(&payload.asset_id)? {
            return Err(ServiceError::NotFoundAsset {
                id: payload.asset_id,
            }
            .into());
        }

        let opt_asset_balance: Option<AssetBalance> = self
            .sdk
            .get_account_value(&payload.grantor, &payload.asset_id)?;

        if let Some(v) = opt_asset_balance {
            let allowance = v.allowance.get(&payload.grantee).unwrap_or(&0);

            Ok(GetAllowanceResponse {
                asset_id: payload.asset_id,
                grantor:  payload.grantor,
                grantee:  payload.grantee,
                value:    *allowance,
            })
        } else {
            Ok(GetAllowanceResponse {
                asset_id: payload.asset_id,
                grantor:  payload.grantor,
                grantee:  payload.grantee,
                value:    0,
            })
        }
    }

    #[cycles(210_00)]
    #[write]
    fn create_asset(
        &mut self,
        ctx: ServiceContext,
        payload: CreateAssetPayload,
    ) -> ProtocolResult<Asset> {
        let caller = ctx.get_caller();
        let payload_str = serde_json::to_string(&payload).map_err(ServiceError::JsonParse)?;

        let id = Hash::digest(Bytes::from(payload_str + &caller.as_hex()));

        if self.assets.contains(&id)? {
            return Err(ServiceError::Exists { id }.into());
        }
        let asset = Asset {
            id:     id.clone(),
            name:   payload.name,
            symbol: payload.symbol,
            supply: payload.supply,
            issuer: caller,
        };
        self.assets.insert(id, asset.clone())?;

        let asset_balance = AssetBalance {
            value:     payload.supply,
            allowance: BTreeMap::new(),
        };

        self.sdk
            .set_account_value(&asset.issuer, asset.id.clone(), asset_balance)?;

        let event_str = serde_json::to_string(&asset).map_err(ServiceError::JsonParse)?;
        ctx.emit_event(event_str)?;

        Ok(asset)
    }

    #[cycles(210_00)]
    #[write]
    fn transfer(&mut self, ctx: ServiceContext, payload: TransferPayload) -> ProtocolResult<()> {
        let caller = ctx.get_caller();
        let asset_id = payload.asset_id.clone();
        let value = payload.value;
        let to = payload.to;

        if !self.assets.contains(&asset_id)? {
            return Err(ServiceError::NotFoundAsset { id: asset_id }.into());
        }

        self._transfer(caller.clone(), to.clone(), asset_id.clone(), value)?;

        let event = TransferEvent {
            asset_id,
            from: caller,
            to,
            value,
        };
        let event_str = serde_json::to_string(&event).map_err(ServiceError::JsonParse)?;
        ctx.emit_event(event_str)
    }

    #[cycles(210_00)]
    #[write]
    fn approve(&mut self, ctx: ServiceContext, payload: ApprovePayload) -> ProtocolResult<()> {
        let caller = ctx.get_caller();
        let asset_id = payload.asset_id.clone();
        let value = payload.value;
        let to = payload.to;

        if !self.assets.contains(&asset_id)? {
            return Err(ServiceError::NotFoundAsset { id: asset_id }.into());
        }

        let mut caller_asset_balance: AssetBalance = self
            .sdk
            .get_account_value(&caller, &asset_id)?
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
            .set_account_value(&caller, asset_id.clone(), caller_asset_balance)?;

        let event = ApproveEvent {
            asset_id,
            grantor: caller,
            grantee: to,
            value,
        };
        let event_str = serde_json::to_string(&event).map_err(ServiceError::JsonParse)?;
        ctx.emit_event(event_str)
    }

    #[cycles(210_00)]
    #[write]
    fn transfer_from(
        &mut self,
        ctx: ServiceContext,
        payload: TransferFromPayload,
    ) -> ProtocolResult<()> {
        let caller = ctx.get_caller();
        let sender = payload.sender;
        let recipient = payload.recipient;
        let asset_id = payload.asset_id;
        let value = payload.value;

        if !self.assets.contains(&asset_id)? {
            return Err(ServiceError::NotFoundAsset { id: asset_id }.into());
        }

        let mut sender_asset_balance: AssetBalance = self
            .sdk
            .get_account_value(&sender, &asset_id)?
            .unwrap_or(AssetBalance {
                value:     0,
                allowance: BTreeMap::new(),
            });
        let sender_allowance = sender_asset_balance
            .allowance
            .entry(caller.clone())
            .or_insert(0);
        if *sender_allowance < value {
            return Err(ServiceError::LackOfBalance {
                expect: value,
                real:   *sender_allowance,
            }
            .into());
        }
        let after_sender_allowance = *sender_allowance - value;
        sender_asset_balance
            .allowance
            .entry(caller.clone())
            .and_modify(|e| *e = after_sender_allowance)
            .or_insert(after_sender_allowance);
        self.sdk
            .set_account_value(&sender, asset_id.clone(), sender_asset_balance)?;

        self._transfer(sender.clone(), recipient.clone(), asset_id.clone(), value)?;

        let event = TransferFromEvent {
            asset_id,
            caller,
            sender,
            recipient,
            value,
        };
        let event_str = serde_json::to_string(&event).map_err(ServiceError::JsonParse)?;
        ctx.emit_event(event_str)
    }

    fn _transfer(
        &mut self,
        sender: Address,
        recipient: Address,
        asset_id: Hash,
        value: u64,
    ) -> ProtocolResult<()> {
        let mut sender_asset_balance: AssetBalance = self
            .sdk
            .get_account_value(&sender, &asset_id)?
            .unwrap_or(AssetBalance {
                value:     0,
                allowance: BTreeMap::new(),
            });
        let sender_balance = sender_asset_balance.value;

        if sender_balance < value {
            return Err(ServiceError::LackOfBalance {
                expect: value,
                real:   sender_balance,
            }
            .into());
        }

        let mut to_asset_balance: AssetBalance = self
            .sdk
            .get_account_value(&recipient, &asset_id)?
            .unwrap_or(AssetBalance {
                value:     0,
                allowance: BTreeMap::new(),
            });

        let (v, overflow) = to_asset_balance.value.overflowing_add(value);
        if overflow {
            return Err(ServiceError::U64Overflow.into());
        }
        to_asset_balance.value = v;

        self.sdk
            .set_account_value(&recipient, asset_id.clone(), to_asset_balance)?;

        let (v, overflow) = sender_balance.overflowing_sub(value);
        if overflow {
            return Err(ServiceError::U64Overflow.into());
        }
        sender_asset_balance.value = v;
        self.sdk
            .set_account_value(&sender, asset_id, sender_asset_balance)?;

        Ok(())
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceError {
    #[display(fmt = "Parsing payload to json failed {:?}", _0)]
    JsonParse(serde_json::Error),

    #[display(fmt = "Asset {:?} already exists", id)]
    Exists {
        id: Hash,
    },

    #[display(fmt = "Not found asset, id {:?}", id)]
    NotFoundAsset {
        id: Hash,
    },

    #[display(fmt = "Not found asset, expect {:?} real {:?}", expect, real)]
    LackOfBalance {
        expect: u64,
        real:   u64,
    },

    U64Overflow,
}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for ProtocolError {
    fn from(err: ServiceError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
