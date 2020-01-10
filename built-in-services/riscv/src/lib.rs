#[cfg(test)]
mod tests;
pub mod types;

use std::io::Read;

use bytes::Bytes;
use derive_more::{Display, From};

use binding_macro::{cycles, service, write};
use protocol::traits::{ServiceSDK, StoreMap};
use protocol::types::{Address, Hash, ServiceContext};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::types::{Contract, DeployPayload, ExecPayload, ExecResp};

pub struct RiscvService<SDK> {
    sdk: SDK,
    // assets: Box<dyn StoreMap<Hash, Asset>>,
}

#[service]
impl<SDK: ServiceSDK> RiscvService<SDK> {
    pub fn init(mut sdk: SDK) -> ProtocolResult<Self> {
        Ok(Self { sdk })
    }

    #[cycles(200_00)]
    #[write]
    fn exec(&mut self, ctx: ServiceContext, payload: ExecPayload) -> ProtocolResult<ExecResp> {
        // let mut file =
        // std::fs::File::open("built-in-services/riscv/src/tests/is13").unwrap();
        // let mut buffer = Vec::new();
        // file.read_to_end(&mut buffer).unwrap();
        // let buffer = Bytes::from(buffer);
        // let contract_code =
        // self.sdk
        //     .get_value<Bytes>(Bytes::from(code_hash.as_hex() + ":code"))?;
        let mut exec_resp = ExecResp::default();
        let contract = self
            .sdk
            .get_value::<Address, Contract>(&payload.address)?
            .unwrap();
        // .ok_or(Err(ServiceError::ContractNotExists).into())?;
        // if contract.is_none() {
        //     return Err(ServiceError::ContractNotExist).into();
        // };
        // let contract = contract.unwrap();
        let code: Bytes = self
            .sdk
            .get_value::<Hash, Bytes>(&contract.code_hash)?
            .unwrap();
        let args: Vec<Bytes> = vec!["a.out".into(), payload.args];
        let r = ckb_vm::run::<u64, ckb_vm::SparseMemory<u64>>(&code, &args[..]);
        dbg!(&r);
        match r {
            Err(e) => exec_resp.error = Some(format!("{}", e)),
            Ok(ret_code) => exec_resp.ret_code = ret_code,
        }
        Ok(exec_resp)
    }

    #[cycles(210_00)]
    #[write]
    fn deploy(&mut self, ctx: ServiceContext, payload: DeployPayload) -> ProtocolResult<String> {
        dbg!(&payload);
        let code_hash = Hash::digest(payload.code.clone());
        self.sdk.set_value(code_hash.clone(), payload.code)?;
        let contract_address = Address::from_bytes(
            Hash::digest(Bytes::from(ctx.get_caller().as_hex() + "nonce"))
                .as_bytes()
                .slice(0..20),
        )?;
        self.sdk
            .set_value(contract_address.clone(), Contract { code_hash })?;
        Ok(contract_address.as_hex())
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceError {
    #[display(fmt = "Contract not exists")]
    ContractNotExists,
}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for ProtocolError {
    fn from(err: ServiceError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
