use std::convert::TryFrom;

use bytes::Bytes;
use prost::{Message, Oneof};

use crate::{
    codec::{
        primitive::{AssetID, Balance, ContractAddress, ContractType, Fee, Hash, UserAddress},
        CodecError, ProtocolCodecSync,
    },
    field, impl_default_bytes_codec_for,
    types::primitive as protocol_primitive,
    types::Bloom,
    ProtocolError, ProtocolResult,
};

// #####################
// Protobuf
// #####################

#[derive(Clone, Message)]
pub struct Receipt {
    #[prost(message, tag = "1")]
    pub state_root: Option<Hash>,

    #[prost(uint64, tag = "2")]
    pub epoch_id: u64,

    #[prost(message, tag = "3")]
    pub tx_hash: Option<Hash>,

    #[prost(message, tag = "4")]
    pub cycles_used: Option<Fee>,

    #[prost(oneof = "ReceiptResult", tags = "5, 6, 7, 8, 9")]
    pub result: Option<ReceiptResult>,
}

#[derive(Clone, Oneof)]
pub enum ReceiptResult {
    #[prost(message, tag = "5")]
    Transfer(Transfer),
    #[prost(message, tag = "6")]
    Approve(Approve),
    #[prost(message, tag = "7")]
    Deploy(Deploy),
    #[prost(message, tag = "8")]
    Call(Call),
    #[prost(message, tag = "9")]
    Fail(Fail),
}

#[derive(Clone, Message)]
pub struct Transfer {
    #[prost(message, tag = "1")]
    pub receiver: Option<UserAddress>,
    #[prost(message, tag = "2")]
    pub before_amount: Option<Balance>,
    #[prost(message, tag = "3")]
    pub after_amount: Option<Balance>,
}

#[derive(Clone, Message)]
pub struct Approve {
    #[prost(message, tag = "1")]
    pub spender: Option<ContractAddress>,
    #[prost(message, tag = "2")]
    pub asset_id: Option<AssetID>,
    #[prost(message, tag = "3")]
    pub max: Option<Balance>,
}

#[derive(Clone, Message)]
pub struct Deploy {
    #[prost(message, tag = "1")]
    pub contract: Option<ContractAddress>,
    #[prost(enumeration = "ContractType", tag = "2")]
    pub contract_type: i32,
}

#[derive(Clone, Message)]
pub struct Call {
    #[prost(message, tag = "1")]
    pub contract: Option<ContractAddress>,
    #[prost(bytes, tag = "2")]
    pub return_value: Vec<u8>,
    #[prost(bytes, tag = "3")]
    pub logs_bloom: Vec<u8>,
}

#[derive(Clone, Message)]
pub struct Fail {
    #[prost(string, tag = "1")]
    pub system: String,
    #[prost(string, tag = "2")]
    pub user: String,
}

// #################
// Conversion
// #################

// ReceiptResult

impl From<receipt::ReceiptResult> for ReceiptResult {
    fn from(result: receipt::ReceiptResult) -> ReceiptResult {
        match result {
            receipt::ReceiptResult::Transfer {
                receiver,
                before_amount,
                after_amount,
            } => {
                let transfer = Transfer {
                    receiver:      Some(UserAddress::from(receiver)),
                    before_amount: Some(Balance::from(before_amount)),
                    after_amount:  Some(Balance::from(after_amount)),
                };

                ReceiptResult::Transfer(transfer)
            }
            receipt::ReceiptResult::Approve {
                spender,
                asset_id,
                max,
            } => {
                let approve = Approve {
                    spender:  Some(ContractAddress::from(spender)),
                    asset_id: Some(AssetID::from(asset_id)),
                    max:      Some(Balance::from(max)),
                };

                ReceiptResult::Approve(approve)
            }
            receipt::ReceiptResult::Deploy {
                contract,
                contract_type,
            } => {
                let deploy = Deploy {
                    contract:      Some(ContractAddress::from(contract)),
                    contract_type: contract_type as i32,
                };

                ReceiptResult::Deploy(deploy)
            }
            receipt::ReceiptResult::Call {
                contract,
                return_value,
                logs_bloom,
            } => {
                let call = Call {
                    contract:     Some(ContractAddress::from(contract)),
                    return_value: return_value.to_vec(),
                    logs_bloom:   logs_bloom.as_bytes().to_vec(),
                };

                ReceiptResult::Call(call)
            }
            receipt::ReceiptResult::Fail { system, user } => {
                let fail = Fail { system, user };

                ReceiptResult::Fail(fail)
            }
        }
    }
}

impl TryFrom<ReceiptResult> for receipt::ReceiptResult {
    type Error = ProtocolError;

    fn try_from(result: ReceiptResult) -> Result<receipt::ReceiptResult, Self::Error> {
        match result {
            ReceiptResult::Transfer(transfer) => {
                let receiver = field!(transfer.receiver, "ReceiptResult::Transfer", "receiver")?;
                let before_amount = field!(
                    transfer.before_amount,
                    "ReceiptResult::Transfer",
                    "before_amount"
                )?;
                let after_amount = field!(
                    transfer.after_amount,
                    "ReceiptResult::Transfer",
                    "after_amount"
                )?;

                let result = receipt::ReceiptResult::Transfer {
                    receiver:      protocol_primitive::UserAddress::try_from(receiver)?,
                    before_amount: protocol_primitive::Balance::try_from(before_amount)?,
                    after_amount:  protocol_primitive::Balance::try_from(after_amount)?,
                };

                Ok(result)
            }
            ReceiptResult::Approve(approve) => {
                let spender = field!(approve.spender, "ReceiptResult::Approve", "spender")?;
                let asset_id = field!(approve.asset_id, "ReceiptResult::Approve", "asset_id")?;
                let max = field!(approve.max, "ReceiptResult::Approve", "max")?;

                let result = receipt::ReceiptResult::Approve {
                    spender:  protocol_primitive::ContractAddress::try_from(spender)?,
                    asset_id: protocol_primitive::AssetID::try_from(asset_id)?,
                    max:      protocol_primitive::Balance::try_from(max)?,
                };

                Ok(result)
            }
            ReceiptResult::Deploy(deploy) => {
                let contract = field!(deploy.contract, "ReceiptResult::Deploy", "contract")?;

                let contract_type = match deploy.contract_type {
                    0 => protocol_primitive::ContractType::Asset,
                    1 => protocol_primitive::ContractType::Library,
                    2 => protocol_primitive::ContractType::App,
                    _ => return Err(CodecError::InvalidContractType(deploy.contract_type).into()),
                };

                let result = receipt::ReceiptResult::Deploy {
                    contract: protocol_primitive::ContractAddress::try_from(contract)?,
                    contract_type,
                };

                Ok(result)
            }
            ReceiptResult::Call(call) => {
                let contract = field!(call.contract, "ReceiptResult::Call", "contract")?;

                let action = receipt::ReceiptResult::Call {
                    contract:     protocol_primitive::ContractAddress::try_from(contract)?,
                    return_value: Bytes::from(call.return_value),
                    logs_bloom:   Box::new(Bloom::from_slice(&call.logs_bloom)),
                };

                Ok(action)
            }
            ReceiptResult::Fail(fail) => {
                let action = receipt::ReceiptResult::Fail {
                    system: fail.system,
                    user:   fail.user,
                };

                Ok(action)
            }
        }
    }
}

// Receipt

impl From<receipt::Receipt> for Receipt {
    fn from(receipt: receipt::Receipt) -> Receipt {
        let state_root = Some(Hash::from(receipt.state_root));
        let tx_hash = Some(Hash::from(receipt.tx_hash));
        let cycles_used = Some(Fee::from(receipt.cycles_used));
        let result = Some(ReceiptResult::from(receipt.result));

        Receipt {
            state_root,
            epoch_id: receipt.epoch_id,
            tx_hash,
            cycles_used,
            result,
        }
    }
}

impl TryFrom<Receipt> for receipt::Receipt {
    type Error = ProtocolError;

    fn try_from(receipt: Receipt) -> Result<receipt::Receipt, Self::Error> {
        let state_root = field!(receipt.state_root, "Receipt", "state_root")?;
        let tx_hash = field!(receipt.tx_hash, "Receipt", "tx_hash")?;
        let cycles_used = field!(receipt.cycles_used, "Receipt", "cycles_used")?;
        let result = field!(receipt.result, "Receipt", "result")?;

        let receipt = receipt::Receipt {
            state_root:  protocol_primitive::Hash::try_from(state_root)?,
            epoch_id:    receipt.epoch_id,
            tx_hash:     protocol_primitive::Hash::try_from(tx_hash)?,
            cycles_used: protocol_primitive::Fee::try_from(cycles_used)?,
            result:      receipt::ReceiptResult::try_from(result)?,
        };

        Ok(receipt)
    }
}

// #################
// Codec
// #################

impl_default_bytes_codec_for!(receipt, [Receipt]);
