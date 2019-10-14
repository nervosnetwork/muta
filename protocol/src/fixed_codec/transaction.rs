use bytes::Bytes;

use crate::{ProtocolResult, impl_default_fixed_codec_for};
use crate::fixed_codec::{FixedCodecError, ProtocolFixedCodec, bytes_to_u64};
use crate::types::{
    transaction::{RawTransaction, TransactionAction, SignedTransaction, CarryingAsset},
    primitive::{ContractType, Hash, Balance, UserAddress, ContractAddress, Fee},
};

impl_default_fixed_codec_for!(transaction, [RawTransaction, SignedTransaction]);

impl rlp::Encodable for RawTransaction {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(5)
            .append(&self.action)
            .append(&self.chain_id.as_bytes().to_vec())
            .append(&self.fee)
            .append(&self.nonce.as_bytes().to_vec())
            .append(&self.timeout.to_be_bytes().to_vec());
    }
}

impl rlp::Decodable for RawTransaction {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 5 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let mut values = Vec::with_capacity(5);

        for val in r {
            let data = val.data()?;
            values.push(data)
        }

        let action: TransactionAction = rlp::decode(r.at(0)?.as_raw())?;
        let chain_id = Hash::from_bytes(Bytes::from(values[1])).map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let fee: Fee = rlp::decode(r.at(2)?.as_raw())?;
        let nonce = Hash::from_bytes(Bytes::from(values[3])).map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let timeout = bytes_to_u64(values[4]);

        Ok(RawTransaction {
            chain_id,
            nonce,
            timeout,
            fee,
            action
        })
    }
}

impl rlp::Encodable for SignedTransaction {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(4)
            .append(&self.pubkey.to_vec())
            .append(&self.raw)
            .append(&self.signature.to_vec())
            .append(&self.tx_hash.as_bytes().to_vec());
    }
}

impl rlp::Decodable for SignedTransaction {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 4 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let mut values = Vec::with_capacity(4);

        for val in r {
            let data = val.data()?;
            values.push(data)
        }

        let pubkey = Bytes::from(values[0]);
        let raw: RawTransaction = rlp::decode(r.at(1)?.as_raw())?;
        let signature = Bytes::from(values[2]);
        let tx_hash = Hash::from_bytes(Bytes::from(values[3])).map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(SignedTransaction {
            raw,
            tx_hash,
            pubkey,
            signature
        })
    }
}

impl rlp::Encodable for CarryingAsset {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2)
            .append(&self.amount.to_bytes_be())
            .append(&self.asset_id.as_bytes().to_vec());
    }
}

impl rlp::Decodable for CarryingAsset {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let amount = Balance::from_bytes_be(r.at(0)?.data()?);
        let asset_id = Hash::from_bytes(Bytes::from(r.at(1)?.data()?)).map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        Ok(CarryingAsset {
            asset_id,
            amount
        })
    }
}

const TRANSFER_ACTION_FLAG: u8 = 0;
const APPROVE_ACTION_FLAG: u8 = 1;
const DEPLOY_ACTION_FLAG: u8 = 2;
const CALL_ACTION_FLAG: u8 = 3;

impl rlp::Encodable for TransactionAction {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        match self {
            TransactionAction::Transfer {
                receiver,
                carrying_asset,
            } => {
                s.begin_list(3)
                    .append(&TRANSFER_ACTION_FLAG)
                    .append(carrying_asset)
                    .append(&receiver.as_bytes().to_vec());
            }
            TransactionAction::Approve {
                spender,
                asset_id,
                max,
            } => {
                s.begin_list(4)
                    .append(&APPROVE_ACTION_FLAG)
                    .append(&asset_id.as_bytes().to_vec())
                    .append(&max.to_bytes_be())
                    .append(&spender.as_bytes().to_vec());
            }
            TransactionAction::Deploy {
                code,
                contract_type,
            } => {
                s.begin_list(3);
                s.append(&DEPLOY_ACTION_FLAG);
                s.append(&code.to_vec());

                let type_flag: u8 = match contract_type {
                    ContractType::Asset => 0,
                    ContractType::App => 1,
                    ContractType::Library => 2,
                    ContractType::Native => 3,
                };
                s.append(&type_flag);
            }
            TransactionAction::Call { .. } => {
                // TODO(@yejiayu): The interface for `call` is about to be modified.
                unimplemented!()
            }
        }
    }
}

impl rlp::Decodable for TransactionAction {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let flag: u8 = r.at(0)?.as_val()?;

        match flag {
            TRANSFER_ACTION_FLAG => {
                let carrying_asset: CarryingAsset = rlp::decode(r.at(1)?.as_raw())?;
                let receiver: UserAddress = UserAddress::from_bytes(Bytes::from(r.at(2)?.data()?))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

                Ok(TransactionAction::Transfer{
                    receiver,
                    carrying_asset
                })
            }
            APPROVE_ACTION_FLAG => {
                let asset_id = Hash::from_bytes(Bytes::from(r.at(1)?.data()?))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                let max = Balance::from_bytes_be(r.at(2)?.data()?);
                let spender = ContractAddress::from_bytes(Bytes::from(r.at(3)?.data()?))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                    
                Ok(TransactionAction::Approve{
                    spender,
                    asset_id,
                    max
                })
            }
            DEPLOY_ACTION_FLAG => {
                let code = Bytes::from(r.at(1)?.data()?);
                let contract_type_flag: u8 = r.at(2)?.as_val()?;
                let contract_type = match contract_type_flag {
                    0 => ContractType::Asset,
                    1 => ContractType::App,
                    2 => ContractType::Library,
                    3 => ContractType::Native,
                    _ => return Err(rlp::DecoderError::Custom("invalid contract type flag")),
                };

                Ok(TransactionAction::Deploy{
                    code,
                    contract_type
                })
            }
            CALL_ACTION_FLAG => {
                // TODO(@yejiayu): The interface for `call` is about to be modified.
                unimplemented!()
            }
            _ => Err(rlp::DecoderError::RlpListLenWithZeroPrefix)
        }
    }
}
