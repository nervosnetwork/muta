#[cfg(test)]
mod tests;
mod types;

use bytes::Bytes;
use derive_more::{Display, From};

use binding_macro::{cycles, init, read, service, write};
use protocol::traits::{RequestContext, ReturnEmpty, ServiceSDK, StoreMap, RETURN_EMPTY};
use protocol::types::Hash;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::types::{
    Asset, CreateAssetPayload, GetAssetPayload, GetBalancePayload, GetBalanceResponse,
    TransferPayload,
};

pub struct AssetService<SDK> {
    sdk:    SDK,
    assets: Box<dyn StoreMap<Hash, Asset>>,
}

#[service]
impl<SDK: ServiceSDK> AssetService<SDK> {
    #[init]
    fn init(mut sdk: SDK) -> ProtocolResult<Self> {
        let assets: Box<dyn StoreMap<Hash, Asset>> = sdk.alloc_or_recover_map("assrts")?;

        Ok(Self { assets, sdk })
    }

    #[cycles(100_00)]
    #[read]
    fn get_asset<Context: RequestContext>(
        &self,
        _ctx: Context,
        payload: GetAssetPayload,
    ) -> ProtocolResult<Asset> {
        let asset = self.assets.get(&payload.id)?;
        Ok(asset)
    }

    #[cycles(100_00)]
    #[read]
    fn get_balance<Context: RequestContext>(
        &self,
        ctx: Context,
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
    fn create_asset<Context: RequestContext>(
        &mut self,
        ctx: Context,
        payload: CreateAssetPayload,
    ) -> ProtocolResult<Asset> {
        let caller = ctx.get_caller();
        let payload_str =
            serde_json::to_string(&payload).map_err(|e| ServiceError::JsonParse(e))?;

        let id = Hash::digest(Bytes::from(payload_str + &caller.as_hex()));

        if self.assets.contains(&id)? {
            return Err(ServiceError::Exists { id }.into());
        }
        let asset = Asset {
            id:     id.clone(),
            name:   payload.name,
            symbol: payload.symbol,
            supply: payload.supply,
            owner:  caller.clone(),
        };
        self.assets.insert(id.clone(), asset.clone())?;

        self.sdk
            .set_account_value(&caller, id.clone(), payload.supply)?;

        Ok(asset)
    }

    #[cycles(210_00)]
    #[write]
    fn transfer<Context: RequestContext>(
        &mut self,
        ctx: Context,
        payload: TransferPayload,
    ) -> ProtocolResult<ReturnEmpty> {
        let caller = ctx.get_caller();
        let asset_id = payload.asset_id.clone();
        let value = payload.value;
        let to = payload.to.clone();

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

        let to_balance = self.sdk.get_account_value(&to, &asset_id)?.unwrap_or(0);
        self.sdk
            .set_account_value(&to, asset_id.clone(), to_balance + value)?;
        self.sdk
            .set_account_value(&caller, asset_id.clone(), caller_balance - value)?;

        Ok(RETURN_EMPTY)
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceError {
    #[display(fmt = "Parsing payload to json failed {:?}", _0)]
    JsonParse(serde_json::Error),

    #[display(fmt = "Asset {:?} already exists", id)]
    Exists { id: Hash },

    #[display(fmt = "Not found asset, id {:?}", id)]
    NotFoundAsset { id: Hash },

    #[display(fmt = "Not found asset, expect {:?} real {:?}", expect, real)]
    LackOfBalance { expect: u64, real: u64 },
}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for ProtocolError {
    fn from(err: ServiceError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
