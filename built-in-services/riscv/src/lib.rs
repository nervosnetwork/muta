#[cfg(test)]
mod tests;
pub mod types;
pub mod vm;

use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;

use bytes::Bytes;
use derive_more::{Display, From};

use binding_macro::{cycles, service, write};
use protocol::traits::{ServiceSDK, StoreMap};
use protocol::types::{Address, Hash, ServiceContext};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::types::{Contract, DeployPayload, ExecPayload, ExecResp};
use crate::vm::{ChainInterface, Interpreter, InterpreterConf, InterpreterParams};

pub struct RiscvService<SDK> {
    sdk: SDK,
}

#[service]
impl<SDK: ServiceSDK> RiscvService<SDK> {
    pub fn init(mut sdk: SDK) -> ProtocolResult<Self> {
        Ok(Self { sdk })
    }

    #[cycles(200_00)]
    #[write]
    fn exec(&mut self, ctx: ServiceContext, payload: ExecPayload) -> ProtocolResult<ExecResp> {
        let mut exec_resp = ExecResp::default();
        let contract = self
            .sdk
            .get_value::<Address, Contract>(&payload.address)?
            .unwrap();
        let code: Bytes = self
            .sdk
            .get_value::<Hash, Bytes>(&contract.code_hash)?
            .unwrap();
        let interpreter_params = InterpreterParams {
            code,
            args: payload.args.clone(),
        };
        let mut interpreter = Interpreter::new(
            ctx.clone(),
            InterpreterConf::default(),
            interpreter_params,
            Rc::new(RefCell::new(ChainInterfaceImpl::default())),
        );
        let r = interpreter.run().map_err(|e| ServiceError::CkbVm(e))?;
        dbg!(&r);
        if r.ret_code != 0 {
            exec_resp.is_error = true;
        }
        exec_resp.ret = String::from_utf8_lossy(r.ret.as_ref()).to_string();
        Ok(exec_resp)
    }

    #[cycles(210_00)]
    #[write]
    fn deploy(&mut self, ctx: ServiceContext, payload: DeployPayload) -> ProtocolResult<String> {
        // dbg!(&payload);
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

#[derive(Default)]
struct ChainInterfaceImpl {
    store: ::std::collections::HashMap<Bytes, Bytes>,
}
impl ChainInterface for ChainInterfaceImpl {
    fn get_storage(&self, key: Bytes) -> ProtocolResult<Bytes> {
        Ok(self.store.get(&key).cloned().unwrap_or_default())
    }

    fn set_storage(&mut self, key: Bytes, val: Bytes) -> ProtocolResult<()> {
        self.store.insert(key, val);
        Ok(())
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceError {
    #[display(fmt = "Contract not exists")]
    ContractNotExists,

    #[display(fmt = "ckb vm error: {:?}", _0)]
    CkbVm(ckb_vm::Error),
}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for ProtocolError {
    fn from(err: ServiceError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
