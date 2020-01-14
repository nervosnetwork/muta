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
    sdk: Rc<RefCell<SDK>>,
}

#[service]
impl<SDK: ServiceSDK + 'static> RiscvService<SDK> {
    pub fn init(mut sdk: SDK) -> ProtocolResult<Self> {
        Ok(Self {
            sdk: Rc::new(RefCell::new(sdk)),
        })
    }

    #[write]
    fn exec(&mut self, ctx: ServiceContext, payload: ExecPayload) -> ProtocolResult<String> {
        let contract = self
            .sdk
            .borrow()
            .get_value::<Address, Contract>(&payload.address)?
            .unwrap();
        let code: Bytes = self
            .sdk
            .borrow()
            .get_value::<Hash, Bytes>(&contract.code_hash)?
            .unwrap();
        let interpreter_params = InterpreterParams {
            address: payload.address.clone(),
            code,
            args: payload.args.clone(),
        };
        let mut interpreter = Interpreter::new(
            ctx.clone(),
            InterpreterConf::default(),
            interpreter_params,
            Rc::new(RefCell::new(ChainInterfaceImpl::new(
                ctx.clone(),
                payload,
                self.sdk.clone(),
            ))),
        );
        let r = interpreter.run().map_err(|e| ServiceError::CkbVm(e))?;
        dbg!(&r);
        let ret = String::from_utf8_lossy(r.ret.as_ref()).to_string();
        if r.ret_code != 0 {
            return Err(ServiceError::NonZeroExitCode {
                exitcode: r.ret_code,
                ret,
            }
            .into());
        }
        ctx.sub_cycles(r.cycles_used)?;
        Ok(ret)
    }

    #[write]
    fn deploy(&mut self, ctx: ServiceContext, payload: DeployPayload) -> ProtocolResult<String> {
        // dbg!(&payload);
        let code_hash = Hash::digest(payload.code.clone());
        let code_len = payload.code.len() as u64;
        self.sdk
            .borrow_mut()
            .set_value(code_hash.clone(), payload.code)?;
        let contract_address = Address::from_bytes(
            Hash::digest(Bytes::from(ctx.get_caller().as_hex() + "nonce"))
                .as_bytes()
                .slice(0..20),
        )?;
        self.sdk
            .borrow_mut()
            .set_value(contract_address.clone(), Contract { code_hash })?;
        ctx.sub_cycles(code_len)?;
        Ok(contract_address.as_hex())
    }
}

struct ChainInterfaceImpl<SDK> {
    ctx:             ServiceContext,
    payload:         ExecPayload,
    sdk:             Rc<RefCell<SDK>>,
    all_cycles_used: u64,
}

impl<SDK: ServiceSDK + 'static> ChainInterfaceImpl<SDK> {
    fn new(ctx: ServiceContext, payload: ExecPayload, sdk: Rc<RefCell<SDK>>) -> Self {
        Self {
            ctx,
            payload,
            sdk,
            all_cycles_used: 0,
        }
    }

    fn contract_key(&self, key: &Bytes) -> Hash {
        let mut contract_key = bytes::BytesMut::from(self.payload.address.as_bytes().as_ref());
        contract_key.extend(key);
        Hash::digest(contract_key.freeze())
    }
}

impl<SDK> ChainInterface for ChainInterfaceImpl<SDK>
where
    SDK: ServiceSDK + 'static,
{
    fn get_storage(&self, key: &Bytes) -> ProtocolResult<Bytes> {
        let contract_key = self.contract_key(key);
        self.sdk
            .borrow()
            .get_value::<Hash, Bytes>(&contract_key)
            .map(|v| v.unwrap_or_default())
    }

    fn set_storage(&mut self, key: Bytes, val: Bytes) -> ProtocolResult<()> {
        let contract_key = self.contract_key(&key);
        self.sdk.borrow_mut().set_value(contract_key, val)
    }

    fn contract_call(
        &mut self,
        address: Address,
        args: Bytes,
        current_cycle: u64,
    ) -> ProtocolResult<(String, u64)> {
        let vm_cycle = current_cycle - self.all_cycles_used;
        self.ctx.sub_cycles(vm_cycle)?;
        let payload = ExecPayload { address, args };
        let payload_str = serde_json::to_string(&payload).map_err(|e| ServiceError::Serde(e))?;
        let call_ret = self
            .sdk
            .borrow_mut()
            .write(&self.ctx, "riscv", "exec", &payload_str)?;
        self.all_cycles_used = self.ctx.get_cycles_used();
        Ok((call_ret, self.all_cycles_used))
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceError {
    #[display(fmt = "Contract not exists")]
    ContractNotExists,

    #[display(fmt = "CKB VM return non zero, exitcode: {}, ret: {}", exitcode, ret)]
    NonZeroExitCode { exitcode: i8, ret: String },

    #[display(fmt = "ckb vm error: {:?}", _0)]
    CkbVm(ckb_vm::Error),

    #[display(fmt = "json serde error: {:?}", _0)]
    Serde(serde_json::error::Error),
}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for ProtocolError {
    fn from(err: ServiceError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
