use bytes::Bytes;

use crate::{ProtocolResult, impl_default_fixed_codec_for};
use crate::fixed_codec::{FixedCodecError, ProtocolFixedCodec};
use crate::types::{
    receipt::{Receipt, ReceiptResult},
    primitive::{Hash, Fee, ContractType, UserAddress, ContractAddress, Balance},
};

impl_default_fixed_codec_for!(receipt, [Receipt, ReceiptResult]);

impl rlp::Encodable for Receipt {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(5)
            .append(&self.cycles_used)
            .append(&self.epoch_id)
            .append(&self.result)
            .append(&self.state_root.as_bytes().to_vec())
            .append(&self.tx_hash.as_bytes().to_vec());
    }
}

impl rlp::Decodable for Receipt {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 5 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let mut values = Vec::with_capacity(5);

        for val in r {
            let data = val.data()?;
            values.push(data)
        }

        let cycles_used: Fee = rlp::decode(r.at(0)?.as_raw())?;
        let epoch_id = r.at(1)?.as_val()?;
        let result: ReceiptResult = rlp::decode(r.at(2)?.as_raw())?;
        let state_root = Hash::from_bytes(Bytes::from(r.at(3)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let tx_hash = Hash::from_bytes(Bytes::from(r.at(4)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(Receipt {
            state_root,
            epoch_id,
            tx_hash,
            cycles_used,
            result
        })
    }
}

const TRANSFER_RESULT_FLAG: u8 = 0;
const APPROVE_RESULT_FLAG: u8 = 1;
const DEPLOY_RESULT_FLAG: u8 = 2;
const CALL_RESULT_FLAG: u8 = 3;
const FAIL_RESULT_FLAG: u8 = 4;

impl rlp::Encodable for ReceiptResult {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        match self {
            ReceiptResult::Transfer {
                receiver,
                asset_id,
                before_amount,
                after_amount
            } => {
                s.begin_list(5)
                    .append(&TRANSFER_RESULT_FLAG)
                    .append(&after_amount.to_bytes_be())
                    .append(&asset_id.as_bytes().to_vec())
                    .append(&before_amount.to_bytes_be())
                    .append(&receiver.as_bytes().to_vec());
            },
            ReceiptResult::Approve {
                spender,
                asset_id,
                max
            } => {
                s.begin_list(4)
                    .append(&APPROVE_RESULT_FLAG)
                    .append(&asset_id.as_bytes().to_vec())
                    .append(&max.to_bytes_be())
                    .append(&spender.as_bytes().to_vec());
            },
            ReceiptResult::Deploy {
                contract,
                contract_type
            } => {
                s.begin_list(3)
                    .append(&DEPLOY_RESULT_FLAG)
                    .append(&contract.as_bytes().to_vec());

                let type_flag: u8 = match &contract_type {
                    ContractType::Asset => 0,
                    ContractType::App => 1,
                    ContractType::Library => 2,
                    ContractType::Native => 3,
                };
                s.append(&type_flag);
            },
            ReceiptResult::Call {
                contract,
                return_value,
                logs_bloom
            } => {
                // TODO(@yejiayu): The interface for `call` is about to be modified.
                unimplemented!()
            }
            ReceiptResult::Fail {
                system,
                user
            } => {
                s.begin_list(3)
                    .append(&FAIL_RESULT_FLAG)
                    .append(&system.as_bytes())
                    .append(&user.as_bytes());    
            }
        }
    }
}

impl rlp::Decodable for ReceiptResult {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let flag: u8 = r.at(0)?.as_val()?;

        match flag {
            TRANSFER_RESULT_FLAG => {
                let after_amount = Balance::from_bytes_be(r.at(1)?.data()?);
                let asset_id = Hash::from_bytes(Bytes::from(r.at(2)?.data()?))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                let before_amount = Balance::from_bytes_be(r.at(3)?.data()?);
                let receiver = UserAddress::from_bytes(Bytes::from(r.at(4)?.data()?))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

                Ok(ReceiptResult::Transfer {
                    receiver,
                    asset_id,
                    before_amount,
                    after_amount
                })
            },
            APPROVE_RESULT_FLAG => {
                let asset_id = Hash::from_bytes(Bytes::from(r.at(1)?.data()?))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                let max = Balance::from_bytes_be(r.at(2)?.data()?);
                let spender = ContractAddress::from_bytes(Bytes::from(r.at(3)?.data()?))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                    
                Ok(ReceiptResult::Approve{
                    spender,
                    asset_id,
                    max
                })
            },
            DEPLOY_RESULT_FLAG => {
                let contract = ContractAddress::from_bytes(Bytes::from(r.at(1)?.data()?))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                let contract_type_flag: u8 = r.at(2)?.as_val()?;
                let contract_type = match contract_type_flag {
                    0 => ContractType::Asset,
                    1 => ContractType::App,
                    2 => ContractType::Library,
                    3 => ContractType::Native,
                    _ => return Err(rlp::DecoderError::Custom("invalid contract type flag")),
                };

                Ok(ReceiptResult::Deploy{
                    contract,
                    contract_type
                })
            },
            CALL_RESULT_FLAG => {
                // TODO(@yejiayu): The interface for `call` is about to be modified.
                unimplemented!()
            },
            FAIL_RESULT_FLAG => {
                let system = String::from_utf8(r.at(1)?.data()?.to_vec())
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                let user = String::from_utf8(r.at(2)?.data()?.to_vec())
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

                Ok(ReceiptResult::Fail {
                    system,
                    user
                })
            },
            _ => Err(rlp::DecoderError::RlpListLenWithZeroPrefix),
        }
    }
}
