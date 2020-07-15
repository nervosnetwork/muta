use std::cmp::Ordering;

use muta_codec_derive::RlpFixedCodec;
use serde::{Deserialize, Serialize};

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::types::{Address, Bytes};
use protocol::ProtocolResult;

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct InitGenesisPayload {
    pub info:                        GovernanceInfo,
    pub tx_fee_inlet_address:        Address,
    pub miner_profit_outlet_address: Address,
    pub miner_charge_map:            Vec<MinerChargeConfig>,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct MinerChargeConfig {
    pub address:              Address,
    pub miner_charge_address: Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default)]
pub struct GovernanceInfo {
    pub admin:                          Address,
    pub tx_failure_fee:                 u64,
    pub tx_floor_fee:                   u64,
    pub profit_deduct_rate_per_million: u64,
    pub tx_fee_discount:                Vec<DiscountLevel>,
    pub miner_benefit:                  u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscountLevel {
    pub threshold:        u64,
    pub discount_percent: u64,
}

impl PartialOrd for DiscountLevel {
    fn partial_cmp(&self, other: &DiscountLevel) -> Option<Ordering> {
        self.threshold.partial_cmp(&other.threshold)
    }
}

impl Ord for DiscountLevel {
    fn cmp(&self, other: &DiscountLevel) -> Ordering {
        self.threshold.cmp(&other.threshold)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RecordProfitEvent {
    pub owner:  Address,
    pub amount: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct AccumulateProfitPayload {
    pub address:            Address,
    pub accumulated_profit: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct HookTransferFromPayload {
    pub sender:    Address,
    pub recipient: Address,
    pub value:     u64,
    pub memo:      String,
}
