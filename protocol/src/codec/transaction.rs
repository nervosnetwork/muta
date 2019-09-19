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
    ProtocolError, ProtocolResult,
};

#[derive(Clone, Message)]
pub struct Transfer {
    #[prost(message, tag = "1")]
    pub receiver: Option<UserAddress>,

    #[prost(message, tag = "2")]
    pub asset_id: Option<AssetID>,

    #[prost(message, tag = "3")]
    pub amount: Option<Balance>,
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
    #[prost(bytes, tag = "1")]
    pub code: Vec<u8>,

    #[prost(enumeration = "ContractType", tag = "2")]
    pub contract_type: i32,
}

#[derive(Clone, Message)]
pub struct Call {
    #[prost(message, tag = "1")]
    pub contract: Option<ContractAddress>,

    #[prost(string, tag = "2")]
    pub method: String,

    #[prost(bytes, repeated, tag = "3")]
    pub args: Vec<Vec<u8>>,

    #[prost(message, tag = "4")]
    pub asset_id: Option<AssetID>,

    #[prost(message, tag = "5")]
    pub amount: Option<Balance>,
}

#[derive(Clone, Oneof)]
pub enum TransactionAction {
    #[prost(message, tag = "5")]
    Transfer(Transfer),

    #[prost(message, tag = "6")]
    Approve(Approve),

    #[prost(message, tag = "7")]
    Deploy(Deploy),

    #[prost(message, tag = "8")]
    Call(Call),
}

#[derive(Clone, Message)]
pub struct RawTransaction {
    #[prost(message, tag = "1")]
    pub chain_id: Option<Hash>,

    #[prost(message, tag = "2")]
    pub nonce: Option<Hash>,

    #[prost(uint64, tag = "3")]
    pub timeout: u64,

    #[prost(message, tag = "4")]
    pub fee: Option<Fee>,

    #[prost(oneof = "TransactionAction", tags = "5, 6, 7, 8")]
    pub action: Option<TransactionAction>,
}

#[derive(Clone, Message)]
pub struct SignedTransaction {
    #[prost(message, tag = "1")]
    pub raw: Option<RawTransaction>,

    #[prost(message, tag = "2")]
    pub tx_hash: Option<Hash>,

    #[prost(bytes, tag = "3")]
    pub pubkey: Vec<u8>,

    #[prost(bytes, tag = "4")]
    pub signature: Vec<u8>,
}

// #################
// Conversion
// #################

// TransactionAction

impl From<transaction::TransactionAction> for TransactionAction {
    fn from(action: transaction::TransactionAction) -> TransactionAction {
        match action {
            transaction::TransactionAction::Transfer {
                receiver,
                asset_id,
                amount,
            } => {
                let transfer = Transfer {
                    receiver: Some(UserAddress::from(receiver)),
                    asset_id: Some(AssetID::from(asset_id)),
                    amount:   Some(Balance::from(amount)),
                };

                TransactionAction::Transfer(transfer)
            }
            transaction::TransactionAction::Approve {
                spender,
                asset_id,
                max,
            } => {
                let approve = Approve {
                    spender:  Some(ContractAddress::from(spender)),
                    asset_id: Some(AssetID::from(asset_id)),
                    max:      Some(Balance::from(max)),
                };

                TransactionAction::Approve(approve)
            }
            transaction::TransactionAction::Deploy {
                code,
                contract_type,
            } => {
                let deploy = Deploy {
                    code:          code.to_vec(),
                    contract_type: contract_type as i32,
                };

                TransactionAction::Deploy(deploy)
            }
            transaction::TransactionAction::Call {
                contract,
                method,
                args,
                asset_id,
                amount,
            } => {
                let args = args.into_iter().map(|arg| arg.to_vec()).collect::<Vec<_>>();

                let call = Call {
                    contract: Some(ContractAddress::from(contract)),
                    method,
                    args,
                    asset_id: Some(AssetID::from(asset_id)),
                    amount: Some(Balance::from(amount)),
                };

                TransactionAction::Call(call)
            }
        }
    }
}

impl TryFrom<TransactionAction> for transaction::TransactionAction {
    type Error = ProtocolError;

    fn try_from(action: TransactionAction) -> Result<transaction::TransactionAction, Self::Error> {
        match action {
            TransactionAction::Transfer(transfer) => {
                let receiver =
                    field!(transfer.receiver, "TransactionAction::Transfer", "receiver")?;
                let asset_id =
                    field!(transfer.asset_id, "TransactionAction::Transfer", "asset_id")?;
                let amount = field!(transfer.amount, "TransactionAction::Transfer", "amount")?;

                let action = transaction::TransactionAction::Transfer {
                    receiver: protocol_primitive::UserAddress::try_from(receiver)?,
                    asset_id: protocol_primitive::AssetID::try_from(asset_id)?,
                    amount:   protocol_primitive::Balance::try_from(amount)?,
                };

                Ok(action)
            }
            TransactionAction::Approve(approve) => {
                let spender = field!(approve.spender, "TransactionAction::Approve", "spender")?;
                let asset_id = field!(approve.asset_id, "TransactionAction::Approve", "asset_id")?;
                let max = field!(approve.max, "TransactionAction::Approve", "max")?;

                let action = transaction::TransactionAction::Approve {
                    spender:  protocol_primitive::ContractAddress::try_from(spender)?,
                    asset_id: protocol_primitive::AssetID::try_from(asset_id)?,
                    max:      protocol_primitive::Balance::try_from(max)?,
                };

                Ok(action)
            }
            TransactionAction::Deploy(deploy) => {
                let contract_type = match deploy.contract_type {
                    0 => protocol_primitive::ContractType::Asset,
                    1 => protocol_primitive::ContractType::Library,
                    2 => protocol_primitive::ContractType::App,
                    _ => return Err(CodecError::InvalidContractType(deploy.contract_type).into()),
                };

                let action = transaction::TransactionAction::Deploy {
                    code: Bytes::from(deploy.code),
                    contract_type,
                };

                Ok(action)
            }
            TransactionAction::Call(call) => {
                let contract = field!(call.contract, "TransactionAction::Call", "contract")?;
                let asset_id = field!(call.asset_id, "Transaction::Call", "asset_id")?;
                let amount = field!(call.amount, "Transaction::Call", "amount")?;
                let args = call.args.into_iter();

                let action = transaction::TransactionAction::Call {
                    contract: protocol_primitive::ContractAddress::try_from(contract)?,
                    method:   call.method,
                    args:     args.map(Bytes::from).collect::<Vec<_>>(),
                    asset_id: protocol_primitive::AssetID::try_from(asset_id)?,
                    amount:   protocol_primitive::Balance::try_from(amount)?,
                };

                Ok(action)
            }
        }
    }
}

// RawTransaction

impl From<transaction::RawTransaction> for RawTransaction {
    fn from(raw: transaction::RawTransaction) -> RawTransaction {
        let chain_id = Some(Hash::from(raw.chain_id));
        let nonce = Some(Hash::from(raw.nonce));
        let fee = Some(Fee::from(raw.fee));
        let action = Some(TransactionAction::from(raw.action));

        RawTransaction {
            chain_id,
            nonce,
            timeout: raw.timeout,
            fee,
            action,
        }
    }
}

impl TryFrom<RawTransaction> for transaction::RawTransaction {
    type Error = ProtocolError;

    fn try_from(raw: RawTransaction) -> Result<transaction::RawTransaction, Self::Error> {
        let chain_id = field!(raw.chain_id, "RawTransaction", "chain_id")?;
        let nonce = field!(raw.nonce, "RawTransaction", "nonce")?;
        let fee = field!(raw.fee, "RawTransaction", "fee")?;
        let action = field!(raw.action, "RawTransaction", "action")?;

        let raw_tx = transaction::RawTransaction {
            chain_id: protocol_primitive::Hash::try_from(chain_id)?,
            nonce:    protocol_primitive::Hash::try_from(nonce)?,
            timeout:  raw.timeout,
            fee:      protocol_primitive::Fee::try_from(fee)?,
            action:   transaction::TransactionAction::try_from(action)?,
        };

        Ok(raw_tx)
    }
}

// SignedTransaction

impl From<transaction::SignedTransaction> for SignedTransaction {
    fn from(stx: transaction::SignedTransaction) -> SignedTransaction {
        let raw = RawTransaction::from(stx.raw);
        let tx_hash = Hash::from(stx.tx_hash);

        SignedTransaction {
            raw:       Some(raw),
            tx_hash:   Some(tx_hash),
            pubkey:    stx.pubkey.to_vec(),
            signature: stx.signature.to_vec(),
        }
    }
}

impl TryFrom<SignedTransaction> for transaction::SignedTransaction {
    type Error = ProtocolError;

    fn try_from(stx: SignedTransaction) -> Result<transaction::SignedTransaction, Self::Error> {
        let raw = field!(stx.raw, "SignedTransaction", "raw")?;
        let tx_hash = field!(stx.tx_hash, "SignedTransaction", "tx_hash")?;

        let stx = transaction::SignedTransaction {
            raw:       transaction::RawTransaction::try_from(raw)?,
            tx_hash:   protocol_primitive::Hash::try_from(tx_hash)?,
            pubkey:    Bytes::from(stx.pubkey),
            signature: Bytes::from(stx.signature),
        };

        Ok(stx)
    }
}

// #################
// Codec
// #################

impl_default_bytes_codec_for!(transaction, [RawTransaction, SignedTransaction]);
