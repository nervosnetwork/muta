use rlp::{Encodable, RlpStream};

use protocol::types::{ContractType, RawTransaction, TransactionAction};

pub struct RlpRawTransaction<'a> {
    pub inner: &'a RawTransaction,
}

const TRANSFER_TRANSACTION_FIELD_LENGTH: usize = 8;
const APPROVE_TRANSACTION_FIELD_LENGTH: usize = 9;
const DEPLOY_TRANSACTION_FIELD_LENGTH: usize = 7;
const CALL_TRANSACTION_FIELD_LENGTH: usize = 10;

impl<'a> Encodable for RlpRawTransaction<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        let inner = &self.inner;
        let list_size = match &inner.action {
            TransactionAction::Transfer { .. } => TRANSFER_TRANSACTION_FIELD_LENGTH,
            TransactionAction::Approve { .. } => APPROVE_TRANSACTION_FIELD_LENGTH,
            TransactionAction::Deploy { .. } => DEPLOY_TRANSACTION_FIELD_LENGTH,
            TransactionAction::Call { .. } => CALL_TRANSACTION_FIELD_LENGTH,
        };

        s.begin_list(list_size);
        s.append(&inner.chain_id.as_bytes().to_vec());
        s.append(&inner.fee.cycle);
        s.append(&inner.fee.asset_id.as_bytes().to_vec());
        s.append(&inner.nonce.as_bytes().to_vec());
        s.append(&inner.timeout);

        match &inner.action {
            TransactionAction::Transfer {
                receiver,
                carrying_asset,
            } => {
                s.append(&carrying_asset.amount.to_bytes_be());
                s.append(&carrying_asset.asset_id.as_bytes().to_vec());
                s.append(&receiver.as_bytes().to_vec());
            }
            TransactionAction::Approve {
                spender,
                asset_id,
                max,
            } => {
                s.append(&asset_id.as_bytes().to_vec());
                s.append(&max.to_bytes_be());
                s.append(&spender.as_bytes().to_vec());
            }
            TransactionAction::Deploy {
                code,
                contract_type,
            } => {
                s.append(&code.to_vec());
                let type_flag: u32 = match contract_type {
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
        };
    }
}
