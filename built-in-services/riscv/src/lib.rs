#[cfg(test)]
mod tests;
pub mod types;
pub mod vm;

use std::cell::RefCell;
use std::rc::Rc;

use derive_more::{Display, From};

use binding_macro::{read, service, write};
use protocol::traits::ExecutorParams;
use protocol::traits::ServiceSDK;
use protocol::types::{Address, Hash, ServiceContext};
use protocol::{Bytes, BytesMut, ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::types::{Contract, DeployPayload, DeployResp, ExecPayload};
use crate::vm::{ChainInterface, Interpreter, InterpreterConf, InterpreterParams};

pub struct RiscvService<SDK> {
    sdk: Rc<RefCell<SDK>>,
}

#[service]
impl<SDK: ServiceSDK + 'static> RiscvService<SDK> {
    pub fn init(sdk: SDK) -> ProtocolResult<Self> {
        Ok(Self {
            sdk: Rc::new(RefCell::new(sdk)),
        })
    }

    fn run(
        &self,
        ctx: ServiceContext,
        payload: ExecPayload,
        is_init: bool,
    ) -> ProtocolResult<String> {
        let contract = self
            .sdk
            .borrow()
            .get_value::<Address, Contract>(&payload.address)?
            .ok_or_else(|| ServiceError::ContractNotExists(payload.address.as_hex()))?;
        let code: Bytes = self
            .sdk
            .borrow()
            .get_value::<Hash, Bytes>(&contract.code_hash)?
            .unwrap();
        let interpreter_params = InterpreterParams {
            address: payload.address.clone(),
            code,
            args: payload.args.clone().into(),
            is_init,
        };
        let mut interpreter = Interpreter::new(
            ctx.clone(),
            InterpreterConf::default(),
            contract.intp_type,
            interpreter_params,
            Rc::new(RefCell::new(ChainInterfaceImpl::new(
                ctx.clone(),
                payload,
                Rc::<RefCell<_>>::clone(&self.sdk),
            ))),
        );

        let r = interpreter.run().map_err(ServiceError::CkbVm)?;
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

    #[read]
    fn call(&self, ctx: ServiceContext, payload: ExecPayload) -> ProtocolResult<String> {
        self.run(ctx, payload, false)
    }

    #[write]
    fn exec(&mut self, ctx: ServiceContext, payload: ExecPayload) -> ProtocolResult<String> {
        self.run(ctx, payload, false)
    }

    #[write]
    fn deploy(
        &mut self,
        ctx: ServiceContext,
        payload: DeployPayload,
    ) -> ProtocolResult<DeployResp> {
        let code = Bytes::from(hex::decode(&payload.code).map_err(ServiceError::HexDecode)?);

        // Save code
        let code_hash = Hash::digest(code.clone());
        let code_len = code.len() as u64;
        // Every bytes cost 10 cycles
        ctx.sub_cycles(code_len * 10)?;
        self.sdk.borrow_mut().set_value(code_hash.clone(), code)?;

        let tx_hash = ctx
            .get_tx_hash()
            .ok_or_else(|| ServiceError::NotInExecContext("riscv deploy".to_owned()))?;

        let contract_address =
            Address::from_bytes(Hash::digest(tx_hash.as_bytes()).as_bytes().slice(0..20))?;

        let intp_type = payload.intp_type;
        let contract = Contract::new(code_hash, intp_type);

        self.sdk
            .borrow_mut()
            .set_value(contract_address.clone(), contract)?;

        // run init
        let init_ret = if !payload.init_args.is_empty() {
            let init_payload = ExecPayload {
                address: contract_address.clone(),
                args:    payload.init_args,
            };

            self.run(ctx, init_payload, true)?
        } else {
            String::new()
        };

        Ok(DeployResp {
            address: contract_address,
            init_ret,
        })
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
        let mut contract_key = BytesMut::from(self.payload.address.as_bytes().as_ref());
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
        let payload = ExecPayload {
            address,
            args: String::from_utf8_lossy(args.as_ref()).to_string(),
        };
        let payload_str = serde_json::to_string(&payload).map_err(ServiceError::Serde)?;
        self.service_call("riscv", "exec", &payload_str, current_cycle)
    }

    fn service_call(
        &mut self,
        service: &str,
        method: &str,
        payload: &str,
        current_cycle: u64,
    ) -> ProtocolResult<(String, u64)> {
        let vm_cycle = current_cycle - self.all_cycles_used;
        self.ctx.sub_cycles(vm_cycle)?;
        let extra = self.payload.address.as_hex();
        let call_ret = self.sdk.borrow_mut().write(
            &self.ctx,
            Some(Bytes::from(extra)),
            service,
            method,
            payload,
        )?;
        self.all_cycles_used = self.ctx.get_cycles_used();
        Ok((call_ret, self.all_cycles_used))
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceError {
    #[display(fmt = "method {} can not be invoke with call", _0)]
    NotInExecContext(String),

    #[display(fmt = "Contract {} not exists", _0)]
    ContractNotExists(String),

    #[display(fmt = "CKB VM return non zero, exitcode: {}, ret: {}", exitcode, ret)]
    NonZeroExitCode { exitcode: i8, ret: String },

    #[display(fmt = "ckb vm error: {:?}", _0)]
    CkbVm(ckb_vm::Error),

    #[display(fmt = "json serde error: {:?}", _0)]
    Serde(serde_json::error::Error),

    #[display(fmt = "hex decode error: {:?}", _0)]
    HexDecode(hex::FromHexError),
}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for ProtocolError {
    fn from(err: ServiceError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
