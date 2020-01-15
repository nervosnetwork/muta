#[cfg(test)]
mod tests;
pub mod types;

use bytes::Bytes;
use derive_more::{Display, From};

use binding_macro::{cycles, genesis, service, write};
use protocol::traits::{ServiceSDK, StoreMap};
use protocol::types::{Hash, ServiceContext};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::types::{
    Asset, CreateAssetPayload, GetAssetPayload, GetBalancePayload, GetBalanceResponse,
    InitGenesisPayload, TransferPayload,
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

        self.sdk
            .set_account_value(&asset.issuer, asset.id, payload.supply)
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
        let balance = self
            .sdk
            .get_account_value(&ctx.get_caller(), &payload.asset_id)?
            .unwrap_or(0);
        Ok(GetBalanceResponse {
            asset_id: payload.asset_id,
            balance,
        })
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
            issuer: caller.clone(),
        };
        self.assets.insert(id.clone(), asset.clone())?;

        self.sdk.set_account_value(&caller, id, payload.supply)?;

        Ok(asset)
    }

    #[cycles(210_00)]
    #[write]
    fn transfer(
        &mut self,
        ctx: ServiceContext,
        payload: TransferPayload,
    ) -> ProtocolResult<serde_json::Value> {
        let caller = ctx.get_caller();
        let asset_id = payload.asset_id.clone();
        let value = payload.value;
        let to = payload.to;

        if !self.assets.contains(&asset_id)? {
            return Err(ServiceError::NotFoundAsset { id: asset_id }.into());
        }

        let caller_balance: u64 = self.sdk.get_account_value(&caller, &asset_id)?.unwrap_or(0);
        if caller_balance < value {
            return Err(ServiceError::LackOfBalance {
                expect: value,
                real:   caller_balance,
            }
            .into());
        }

        let to_balance: u64 = self.sdk.get_account_value(&to, &asset_id)?.unwrap_or(0);
        let (v, overflow) = to_balance.overflowing_add(value);
        if overflow {
            return Err(ServiceError::U64Overflow.into());
        }

        self.sdk.set_account_value(&to, asset_id.clone(), v)?;

        let (v, overflow) = caller_balance.overflowing_sub(value);
        if overflow {
            return Err(ServiceError::U64Overflow.into());
        }
        self.sdk.set_account_value(&caller, asset_id, v)?;

        Ok(serde_json::Value::Null)
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
