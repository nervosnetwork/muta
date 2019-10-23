use bytes::Bytes;

use crate::fixed_codec::{FixedCodecError, ProtocolFixedCodec};
use crate::types::primitive::{Balance, ContractAddress, ContractType, Fee, Hash, UserAddress};
use crate::types::transaction::{
    CarryingAsset, RawTransaction, SignedTransaction, TransactionAction,
};
use crate::{impl_default_fixed_codec_for, ProtocolResult};

// Impl ProtocolFixedCodec trait for types
impl_default_fixed_codec_for!(transaction, [RawTransaction, SignedTransaction]);

const TRANSFER_ACTION_FLAG: u8 = 0;
const DEPLOY_ACTION_FLAG: u8 = 1;
const CALL_ACTION_WITH_ASSET_FLAG: u8 = 2;
const CALL_ACTION_WITHOUT_ASSET_FLAG: u8 = 3;

impl rlp::Encodable for RawTransaction {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        match &self.action {
            TransactionAction::Transfer {
                receiver,
                carrying_asset,
            } => {
                s.begin_list(9);
                s.append(&TRANSFER_ACTION_FLAG);

                // Append tx basic fields
                s.append(&self.chain_id.as_bytes().to_vec());
                s.append(&self.fee.asset_id.as_bytes().to_vec());
                s.append(&self.fee.cycle);
                s.append(&self.nonce.as_bytes().to_vec());
                s.append(&self.timeout);

                // Append tx action fields
                s.append(&carrying_asset.amount.to_bytes_be());
                s.append(&carrying_asset.asset_id.as_bytes().to_vec());
                s.append(&receiver.as_bytes().to_vec());
            }
            TransactionAction::Deploy {
                code,
                contract_type,
            } => {
                s.begin_list(8);
                s.append(&DEPLOY_ACTION_FLAG);

                // Append tx basic fields
                s.append(&self.chain_id.as_bytes().to_vec());
                s.append(&self.fee.asset_id.as_bytes().to_vec());
                s.append(&self.fee.cycle);
                s.append(&self.nonce.as_bytes().to_vec());
                s.append(&self.timeout);

                // Append tx action fields
                s.append(&code.to_vec());

                let type_flag: u8 = match contract_type {
                    ContractType::Asset => 0,
                    ContractType::App => 1,
                    ContractType::Library => 2,
                    ContractType::Native => 3,
                };
                s.append(&type_flag);
            }
            TransactionAction::Call {
                contract,
                method,
                args,
                carrying_asset,
            } => {
                match &carrying_asset {
                    Some(_) => {
                        s.begin_list(11);
                        s.append(&CALL_ACTION_WITH_ASSET_FLAG);
                    }
                    None => {
                        s.begin_list(9);
                        s.append(&CALL_ACTION_WITHOUT_ASSET_FLAG);
                    }
                }

                // Append tx basic fields
                s.append(&self.chain_id.as_bytes().to_vec());
                s.append(&self.fee.asset_id.as_bytes().to_vec());
                s.append(&self.fee.cycle);
                s.append(&self.nonce.as_bytes().to_vec());
                s.append(&self.timeout);

                // Append tx action fields
                let args = args
                    .iter()
                    .map(|arg| hex::encode(arg.to_vec()))
                    .collect::<Vec<_>>();

                s.append_list::<String, String>(&args);

                if let Some(c_a) = &carrying_asset {
                    s.append(&c_a.amount.to_bytes_be());
                    s.append(&c_a.asset_id.as_bytes().to_vec());
                }

                s.append(&contract.as_bytes().to_vec());
                s.append(&method.as_bytes());
            }
            _ => {}
        }
    }
}

impl rlp::Decodable for RawTransaction {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let flag: u8 = r.at(0)?.as_val()?;

        match flag {
            TRANSFER_ACTION_FLAG => {
                // Decode tx basic fields
                let (chain_id, fee, nonce, timeout) = help_decode_raw_tx(r)?;

                // Decode tx action fields
                let action = TransactionAction::Transfer {
                    receiver:       UserAddress::from_bytes(Bytes::from(r.at(8)?.data()?))
                        .map_err(|_| rlp::DecoderError::RlpInvalidLength)?,
                    carrying_asset: CarryingAsset {
                        asset_id: Hash::from_bytes(Bytes::from(r.at(7)?.data()?))
                            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?,
                        amount:   Balance::from_bytes_be(r.at(6)?.data()?),
                    },
                };

                Ok(RawTransaction {
                    chain_id,
                    nonce,
                    timeout,
                    fee,
                    action,
                })
            }
            DEPLOY_ACTION_FLAG => {
                // Decode tx basic fields
                let (chain_id, fee, nonce, timeout) = help_decode_raw_tx(r)?;

                // Decode tx action fields
                let code = Bytes::from(r.at(6)?.data()?);

                let contract_type_flag: u8 = r.at(7)?.as_val()?;
                let contract_type = match contract_type_flag {
                    0 => ContractType::Asset,
                    1 => ContractType::App,
                    2 => ContractType::Library,
                    3 => ContractType::Native,
                    _ => return Err(rlp::DecoderError::Custom("invalid contract type flag")),
                };

                let action = TransactionAction::Deploy {
                    code,
                    contract_type,
                };

                Ok(RawTransaction {
                    chain_id,
                    nonce,
                    timeout,
                    fee,
                    action,
                })
            }
            CALL_ACTION_WITH_ASSET_FLAG | CALL_ACTION_WITHOUT_ASSET_FLAG => {
                // Decode tx basic fields
                let (chain_id, fee, nonce, timeout) = help_decode_raw_tx(r)?;

                // Decode tx action fields
                let args: Vec<String> = rlp::decode_list(r.at(6)?.as_raw());
                let args: Result<Vec<_>, _> = args.iter().map(|arg| hex_to_bytes(&arg)).collect();
                let args = args?;

                if let CALL_ACTION_WITH_ASSET_FLAG = flag {
                    let carrying_asset = CarryingAsset {
                        asset_id: Hash::from_bytes(Bytes::from(r.at(8)?.data()?))
                            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?,
                        amount:   Balance::from_bytes_be(r.at(7)?.data()?),
                    };

                    let contract = ContractAddress::from_bytes(Bytes::from(r.at(9)?.data()?))
                        .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                    let method = String::from_utf8(r.at(10)?.data()?.to_vec())
                        .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

                    let action = TransactionAction::Call {
                        contract,
                        method,
                        args,
                        carrying_asset: Some(carrying_asset),
                    };

                    Ok(RawTransaction {
                        chain_id,
                        nonce,
                        timeout,
                        fee,
                        action,
                    })
                } else {
                    let contract = ContractAddress::from_bytes(Bytes::from(r.at(7)?.data()?))
                        .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
                    let method = String::from_utf8(r.at(8)?.data()?.to_vec())
                        .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

                    let action = TransactionAction::Call {
                        contract,
                        method,
                        args,
                        carrying_asset: None,
                    };

                    Ok(RawTransaction {
                        chain_id,
                        nonce,
                        timeout,
                        fee,
                        action,
                    })
                }
            }
            _ => Err(rlp::DecoderError::RlpListLenWithZeroPrefix),
        }
    }
}

impl rlp::Encodable for SignedTransaction {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(4)
            .append(&self.pubkey.to_vec())
            .append(&self.raw)
            .append(&self.signature.to_vec())
            .append(&self.tx_hash);
    }
}

impl rlp::Decodable for SignedTransaction {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 4 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let pubkey = Bytes::from(r.at(0)?.data()?);
        let raw: RawTransaction = rlp::decode(r.at(1)?.as_raw())?;
        let signature = Bytes::from(r.at(2)?.data()?);
        let tx_hash = rlp::decode(r.at(3)?.as_raw())?;

        Ok(SignedTransaction {
            raw,
            tx_hash,
            pubkey,
            signature,
        })
    }
}

fn help_decode_raw_tx(r: &rlp::Rlp) -> Result<(Hash, Fee, Hash, u64), rlp::DecoderError> {
    let chain_id = Hash::from_bytes(Bytes::from(r.at(1)?.data()?))
        .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

    let fee = Fee {
        asset_id: Hash::from_bytes(Bytes::from(r.at(2)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?,
        cycle:    r.at(3)?.as_val()?,
    };

    let nonce = Hash::from_bytes(Bytes::from(r.at(4)?.data()?))
        .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

    let timeout = r.at(5)?.as_val()?;

    Ok((chain_id, fee, nonce, timeout))
}

fn clean_0x(s: &str) -> &str {
    if s.starts_with("0x") {
        &s[2..]
    } else {
        s
    }
}

fn hex_to_bytes(s: &str) -> Result<Bytes, rlp::DecoderError> {
    let s = clean_0x(s);
    let bytes = hex::decode(s)
        .map_err(|_| rlp::DecoderError::Custom("hex to bytes err when decode raw tx"))?;

    Ok(Bytes::from(bytes))
}
