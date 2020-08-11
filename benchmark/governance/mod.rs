mod types;

use std::cell::RefCell;
use std::convert::From;
use std::rc::Rc;

use bytes::Bytes;
use derive_more::{Display, From};

use binding_macro::{genesis, hook_after, service, tx_hook_after, tx_hook_before};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK, StoreMap};
use protocol::types::{Address, Hash, ServiceContext, ServiceContextParams};

use asset::types::TransferPayload;
use asset::Assets;
use types::{GovernanceInfo, InitGenesisPayload};

const INFO_KEY: &str = "admin";
const TX_FEE_INLET_KEY: &str = "fee_address";
const MINER_PROFIT_OUTLET_KEY: &str = "miner_address";
static ADMISSION_TOKEN: Bytes = Bytes::from_static(b"governance");

lazy_static::lazy_static! {
    pub static ref NATIVE_ASSET_ID: Hash = Hash::from_hex("0xf56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c").unwrap();
}

pub struct GovernanceService<A, SDK> {
    sdk:     SDK,
    profits: Box<dyn StoreMap<Address, u64>>,
    miners:  Box<dyn StoreMap<Address, Address>>,
    asset:   A,
}

#[service]
impl<A: Assets, SDK: ServiceSDK> GovernanceService<A, SDK> {
    pub fn new(mut sdk: SDK, asset: A) -> Self {
        let profits: Box<dyn StoreMap<Address, u64>> = sdk.alloc_or_recover_map("profit");
        let miners: Box<dyn StoreMap<Address, Address>> = sdk.alloc_or_recover_map("miner_address");
        Self {
            sdk,
            profits,
            miners,
            asset,
        }
    }

    #[genesis]
    fn init_genesis(&mut self, payload: InitGenesisPayload) {
        assert!(self.profits.is_empty());

        let mut info = payload.info;
        info.tx_fee_discount.sort();
        self.sdk.set_value(INFO_KEY.to_string(), info);
        self.sdk
            .set_value(TX_FEE_INLET_KEY.to_string(), payload.tx_fee_inlet_address);
        self.sdk.set_value(
            MINER_PROFIT_OUTLET_KEY.to_string(),
            payload.miner_profit_outlet_address,
        );

        for miner in payload.miner_charge_map.into_iter() {
            self.miners
                .insert(miner.address, miner.miner_charge_address);
        }
    }

    #[tx_hook_before]
    fn pledge_fee(&mut self, ctx: ServiceContext) -> ServiceResponse<String> {
        let info = self
            .sdk
            .get_value::<_, GovernanceInfo>(&INFO_KEY.to_owned());
        let tx_fee_inlet_address = self
            .sdk
            .get_value::<_, Address>(&TX_FEE_INLET_KEY.to_owned());

        if info.is_none() || tx_fee_inlet_address.is_none() {
            return ServiceError::MissingInfo.into();
        }

        let info = info.unwrap();
        let tx_fee_inlet_address = tx_fee_inlet_address.unwrap();
        let payload = TransferPayload {
            asset_id: NATIVE_ASSET_ID.clone(),
            to:       tx_fee_inlet_address,
            value:    info.tx_failure_fee,
        };

        // Pledge the tx failure fee before executed the transaction.
        match self.asset.transfer_(&ctx, payload) {
            Ok(_) => ServiceResponse::from_succeed("".to_owned()),
            Err(e) => ServiceResponse::from_error(e.code, e.error_message),
        }
    }

    #[tx_hook_after]
    fn deduct_fee(&mut self, ctx: ServiceContext) -> ServiceResponse<String> {
        let tx_fee_inlet_address = self
            .sdk
            .get_value::<_, Address>(&TX_FEE_INLET_KEY.to_owned());
        if tx_fee_inlet_address.is_none() {
            return ServiceError::MissingInfo.into();
        }

        let tx_fee_inlet_address = tx_fee_inlet_address.unwrap();
        let payload = TransferPayload {
            asset_id: NATIVE_ASSET_ID.clone(),
            to:       tx_fee_inlet_address,
            value:    1,
        };

        match self.asset.transfer_(&ctx, payload) {
            Ok(_) => ServiceResponse::from_succeed("".to_owned()),
            Err(e) => ServiceResponse::from_error(e.code, e.error_message),
        }
    }

    #[hook_after]
    fn handle_miner_profit(&mut self, params: &ExecutorParams) {
        let info = self
            .sdk
            .get_value::<_, GovernanceInfo>(&INFO_KEY.to_owned());

        let sender_address = self
            .sdk
            .get_value::<_, Address>(&MINER_PROFIT_OUTLET_KEY.to_owned());

        if info.is_none() || sender_address.is_none() {
            return;
        }

        let info = info.unwrap();
        let sender_address = sender_address.unwrap();

        let ctx_params = ServiceContextParams {
            tx_hash:         None,
            nonce:           None,
            cycles_limit:    params.cycles_limit,
            cycles_price:    1,
            cycles_used:     Rc::new(RefCell::new(0)),
            caller:          sender_address,
            height:          params.height,
            service_name:    String::new(),
            service_method:  String::new(),
            service_payload: String::new(),
            extra:           Some(ADMISSION_TOKEN.clone()),
            timestamp:       params.timestamp,
            events:          Rc::new(RefCell::new(vec![])),
        };

        let recipient_addr = if let Some(addr) = self.miners.get(&params.proposer) {
            addr
        } else {
            params.proposer.clone()
        };

        let payload = TransferPayload {
            asset_id: NATIVE_ASSET_ID.clone(),
            to:       recipient_addr,
            value:    info.miner_benefit,
        };

        let _ = self
            .asset
            .transfer_(&ServiceContext::new(ctx_params), payload);
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceError {
    NonAuthorized,

    #[display(fmt = "Can not get governance info")]
    MissingInfo,

    #[display(fmt = "calc overflow")]
    Overflow,

    #[display(fmt = "query balance failed")]
    QueryBalance,

    #[display(fmt = "Parsing payload to json failed {:?}", _0)]
    JsonParse(serde_json::Error),
}

impl ServiceError {
    fn code(&self) -> u64 {
        match self {
            ServiceError::NonAuthorized => 101,
            ServiceError::JsonParse(_) => 102,
            ServiceError::MissingInfo => 103,
            ServiceError::Overflow => 104,
            ServiceError::QueryBalance => 105,
        }
    }
}

impl<T: Default> From<ServiceError> for ServiceResponse<T> {
    fn from(err: ServiceError) -> ServiceResponse<T> {
        ServiceResponse::from_error(err.code(), err.to_string())
    }
}
