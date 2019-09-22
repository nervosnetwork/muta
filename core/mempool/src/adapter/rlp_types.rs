use std::iter::once;

use bytes::Bytes;
use protocol::types::{
    Balance, CarryingAsset, ContractAddress, ContractType, Fee, Hash, RawTransaction,
    SignedTransaction, TransactionAction, UserAddress,
};
use rlp::{Encodable, RlpStream};

pub struct RlpHash<'a>(&'a Hash);

impl<'a> Encodable for RlpHash<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.encoder().encode_value(&self.0.as_bytes())
    }
}

pub struct RlpUserAddress<'a>(&'a UserAddress);

impl<'a> Encodable for RlpUserAddress<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.encoder().encode_value(&self.0.as_bytes())
    }
}

pub type RlpAssetID<'a> = RlpHash<'a>;

pub struct RlpBalance<'a>(&'a Balance);

impl<'a> Encodable for RlpBalance<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.encoder().encode_value(&self.0.to_bytes_be());
    }
}

pub struct RlpContractAddress<'a>(&'a ContractAddress);

impl<'a> Encodable for RlpContractAddress<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.encoder().encode_value(&self.0.as_bytes());
    }
}

pub struct RlpContractType(ContractType);

impl Encodable for RlpContractType {
    fn rlp_append(&self, s: &mut RlpStream) {
        let contract_type = match self.0 {
            ContractType::Asset => 1u8,
            ContractType::App => 2u8,
            ContractType::Library => 3u8,
            ContractType::Native => 4u8,
        };

        s.encoder().encode_iter(once(contract_type));
    }
}

pub struct RlpFee<'a>(&'a Fee);

impl<'a> Encodable for RlpFee<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2);
        s.append(&RlpHash(&self.0.asset_id));
        s.append(&self.0.cycle);
    }
}

pub struct RlpCarryingAsset<'a>(&'a CarryingAsset);

impl<'a> Encodable for RlpCarryingAsset<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3);
        s.append(&RlpHash(&self.0.asset_id));
        s.append(&RlpBalance(&self.0.amount));
    }
}

pub struct RlpTransfer<'a> {
    receiver: RlpUserAddress<'a>,
    asset_id: RlpAssetID<'a>,
    amount:   RlpBalance<'a>,
}

impl<'a> Encodable for RlpTransfer<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3);
        s.append(&self.receiver);
        s.append(&self.asset_id);
        s.append(&self.amount);
    }
}

pub struct RlpApprove<'a> {
    spender:  RlpContractAddress<'a>,
    asset_id: RlpAssetID<'a>,
    max:      RlpBalance<'a>,
}

impl<'a> Encodable for RlpApprove<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3);
        s.append(&self.spender);
        s.append(&self.asset_id);
        s.append(&self.max);
    }
}

pub struct RlpDeploy<'a> {
    code:          &'a [u8],
    contract_type: RlpContractType,
}

impl<'a> Encodable for RlpDeploy<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2);
        s.append(&self.code);
        s.append(&self.contract_type);
    }
}

pub struct RlpArgs<'a>(&'a [Bytes]);

impl<'a> Encodable for RlpArgs<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(self.0.len());

        for bytes in self.0.iter() {
            s.append(&bytes.as_ref());
        }
    }
}

pub struct RlpCall<'a> {
    contract:       RlpContractAddress<'a>,
    method:         &'a str,
    args:           RlpArgs<'a>,
    carrying_asset: Option<RlpCarryingAsset<'a>>,
}

impl<'a> Encodable for RlpCall<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.contract);
        s.append(&self.method);
        s.append(&self.args);
        match &self.carrying_asset {
            Some(carrying_asset) => {
                s.begin_list(3);
                s.append(&true);
                s.append(carrying_asset);
            }
            None => {
                s.begin_list(1);
                s.append(&false);
            }
        }
    }
}

pub enum RlpTransactionAction<'a> {
    Transfer(RlpTransfer<'a>),
    Approve(RlpApprove<'a>),
    Deploy(RlpDeploy<'a>),
    Call(RlpCall<'a>),
}

impl<'a> Encodable for RlpTransactionAction<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Self::Transfer(ref t) => s.append(t),
            Self::Approve(ref a) => s.append(a),
            Self::Deploy(ref d) => s.append(d),
            Self::Call(c) => s.append(c),
        };
    }
}

impl<'a> From<&'a TransactionAction> for RlpTransactionAction<'a> {
    fn from(tx_act: &'a TransactionAction) -> Self {
        match tx_act {
            TransactionAction::Transfer {
                receiver,
                carrying_asset,
            } => RlpTransactionAction::Transfer(RlpTransfer {
                receiver: RlpUserAddress(receiver),
                asset_id: RlpHash(&carrying_asset.asset_id),
                amount:   RlpBalance(&carrying_asset.amount),
            }),
            TransactionAction::Approve {
                spender,
                asset_id,
                max,
            } => RlpTransactionAction::Approve(RlpApprove {
                spender:  RlpContractAddress(spender),
                asset_id: RlpHash(&asset_id),
                max:      RlpBalance(max),
            }),
            TransactionAction::Deploy {
                code,
                contract_type,
            } => RlpTransactionAction::Deploy(RlpDeploy {
                code:          code.as_ref(),
                contract_type: RlpContractType(contract_type.clone()),
            }),
            TransactionAction::Call {
                contract,
                method,
                args,
                carrying_asset,
            } => RlpTransactionAction::Call(RlpCall {
                contract: RlpContractAddress(contract),
                method,
                args: RlpArgs(args),
                carrying_asset: carrying_asset.as_ref().map(|a| RlpCarryingAsset(a)),
            }),
        }
    }
}

pub struct RlpRawTransaction<'a> {
    chain_id: RlpHash<'a>,
    nonce:    RlpHash<'a>,
    timeout:  u64,
    fee:      RlpFee<'a>,
    action:   RlpTransactionAction<'a>,
}

impl<'a> Encodable for RlpRawTransaction<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(5);
        s.append(&self.chain_id);
        s.append(&self.nonce);
        s.append(&self.timeout);
        s.append(&self.fee);
        s.append(&self.action);
    }
}

impl<'a> From<&'a RawTransaction> for RlpRawTransaction<'a> {
    fn from(raw_tx: &'a RawTransaction) -> Self {
        RlpRawTransaction {
            chain_id: RlpHash(&raw_tx.chain_id),
            nonce:    RlpHash(&raw_tx.nonce),
            timeout:  raw_tx.timeout,
            fee:      RlpFee(&raw_tx.fee),
            action:   RlpTransactionAction::from(&raw_tx.action),
        }
    }
}

pub struct RlpSignedTransaction<'a> {
    raw:       RlpRawTransaction<'a>,
    tx_hash:   RlpHash<'a>,
    pubkey:    &'a [u8],
    signature: &'a [u8],
}

impl<'a> Encodable for RlpSignedTransaction<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.raw);
        s.append(&self.tx_hash);
        s.append(&self.pubkey);
        s.append(&self.signature);
    }
}

impl<'a> From<&'a SignedTransaction> for RlpSignedTransaction<'a> {
    fn from(stx: &'a SignedTransaction) -> RlpSignedTransaction<'a> {
        RlpSignedTransaction {
            raw:       RlpRawTransaction::from(&stx.raw),
            tx_hash:   RlpHash(&stx.tx_hash),
            pubkey:    &stx.pubkey,
            signature: &stx.signature,
        }
    }
}
