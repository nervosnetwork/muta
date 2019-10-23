use bytes::Bytes;

use crate::fixed_codec::{FixedCodecError, ProtocolFixedCodec};
use crate::types::primitive::{Balance, ContractType, Fee};
use crate::types::receipt::{Receipt, ReceiptResult};
use crate::{impl_default_fixed_codec_for, ProtocolResult};

// Impl ProtocolFixedCodec trait for types
impl_default_fixed_codec_for!(receipt, [Receipt, ReceiptResult]);

impl rlp::Encodable for Receipt {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(5)
            .append(&self.cycles_used)
            .append(&self.epoch_id)
            .append(&self.result)
            .append(&self.state_root)
            .append(&self.tx_hash);
    }
}

impl rlp::Decodable for Receipt {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 5 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let cycles_used: Fee = rlp::decode(r.at(0)?.as_raw())?;
        let epoch_id = r.at(1)?.as_val()?;
        let result: ReceiptResult = rlp::decode(r.at(2)?.as_raw())?;
        let state_root = rlp::decode(r.at(3)?.as_raw())?;
        let tx_hash = rlp::decode(r.at(4)?.as_raw())?;

        Ok(Receipt {
            state_root,
            epoch_id,
            tx_hash,
            cycles_used,
            result,
        })
    }
}

const TRANSFER_RESULT_FLAG: u8 = 0;
const DEPLOY_RESULT_FLAG: u8 = 1;
const CALL_RESULT_FLAG: u8 = 2;
const FAIL_RESULT_FLAG: u8 = 3;

impl rlp::Encodable for ReceiptResult {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        match self {
            ReceiptResult::Transfer {
                receiver,
                asset_id,
                before_amount,
                after_amount,
            } => {
                s.begin_list(5)
                    .append(&TRANSFER_RESULT_FLAG)
                    .append(&after_amount.to_bytes_be())
                    .append(asset_id)
                    .append(&before_amount.to_bytes_be())
                    .append(receiver);
            }
            ReceiptResult::Deploy {
                contract,
                contract_type,
            } => {
                s.begin_list(3).append(&DEPLOY_RESULT_FLAG).append(contract);

                let type_flag: u8 = match &contract_type {
                    ContractType::Asset => 0,
                    ContractType::App => 1,
                    ContractType::Library => 2,
                    ContractType::Native => 3,
                };
                s.append(&type_flag);
            }
            ReceiptResult::Call {
                contract,
                return_value,
                logs_bloom,
            } => {
                s.begin_list(4)
                    .append(&CALL_RESULT_FLAG)
                    .append(contract)
                    .append(logs_bloom.as_ref())
                    .append(&return_value.to_vec());
            }
            ReceiptResult::Fail { system, user } => {
                s.begin_list(3)
                    .append(&FAIL_RESULT_FLAG)
                    .append(&system.as_bytes())
                    .append(&user.as_bytes());
            }
            _ => {}
        }
    }
}

impl rlp::Decodable for ReceiptResult {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let flag: u8 = r.at(0)?.as_val()?;

        match flag {
            TRANSFER_RESULT_FLAG => {
                let after_amount = Balance::from_bytes_be(r.at(1)?.data()?);
                let asset_id = rlp::decode(r.at(2)?.as_raw())?;
                let before_amount = Balance::from_bytes_be(r.at(3)?.data()?);
                let receiver = rlp::decode(r.at(4)?.as_raw())?;

                Ok(ReceiptResult::Transfer {
                    receiver,
                    asset_id,
                    before_amount,
                    after_amount,
                })
            }
            DEPLOY_RESULT_FLAG => {
                let contract = rlp::decode(r.at(1)?.as_raw())?;
                let contract_type_flag: u8 = r.at(2)?.as_val()?;
                let contract_type = match contract_type_flag {
                    0 => ContractType::Asset,
                    1 => ContractType::App,
                    2 => ContractType::Library,
                    3 => ContractType::Native,
                    _ => return Err(rlp::DecoderError::Custom("invalid contract type flag")),
                };

                Ok(ReceiptResult::Deploy {
                    contract,
                    contract_type,
                })
            }
            CALL_RESULT_FLAG => {
                let contract = rlp::decode(r.at(1)?.as_raw())?;
                let bloom = rlp::decode(r.at(2)?.as_raw())?;
                let logs_bloom = Box::new(bloom);
                let return_value = Bytes::from(r.at(3)?.data()?);

                Ok(ReceiptResult::Call {
                    contract,
                    return_value,
                    logs_bloom,
                })
            }
            FAIL_RESULT_FLAG => {
                let system = String::from_utf8(r.at(1)?.data()?.to_vec())
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                let user = String::from_utf8(r.at(2)?.data()?.to_vec())
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

                Ok(ReceiptResult::Fail { system, user })
            }
            _ => Err(rlp::DecoderError::RlpListLenWithZeroPrefix),
        }
    }
}
