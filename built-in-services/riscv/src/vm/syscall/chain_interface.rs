//! Environmental Information
use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;
use ckb_vm::instructions::Register;
use ckb_vm::memory::Memory;
use protocol::types::{Address, Hash, ServiceContext};

use crate::vm::cost_model::CONTRACT_CALL_FIXED_CYCLE;
use crate::vm::syscall::common::get_arr;
use crate::vm::syscall::convention::{
    SYSCODE_CONTRACT_CALL, SYSCODE_GET_STORAGE, SYSCODE_SET_STORAGE,
};
use crate::ChainInterface;
use crate::InterpreterParams;

pub struct SyscallChainInterface {
    chain: Rc<RefCell<dyn ChainInterface>>,
}

impl SyscallChainInterface {
    pub fn new(chain: Rc<RefCell<dyn ChainInterface>>) -> Self {
        Self { chain }
    }

    fn set_bytes<Mac: ckb_vm::SupportMachine>(
        &mut self,
        machine: &mut Mac,
        addr: u64,
        size: u64,
        info: &[u8],
    ) -> Result<(), ckb_vm::Error> {
        machine.memory_mut().store_bytes(addr, info)?;
        machine
            .memory_mut()
            .store_bytes(size, &(info.len() as u64).to_le_bytes())?;
        Ok(())
    }
}

impl<Mac: ckb_vm::SupportMachine> ckb_vm::Syscalls<Mac> for SyscallChainInterface {
    fn initialize(&mut self, _machine: &mut Mac) -> Result<(), ckb_vm::Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, ckb_vm::Error> {
        let code = machine.registers()[ckb_vm::registers::A7].to_u64();
        match code {
            SYSCODE_SET_STORAGE => {
                let k_addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let k_size = machine.registers()[ckb_vm::registers::A1].to_u64();
                let v_addr = machine.registers()[ckb_vm::registers::A2].to_u64();
                let v_size = machine.registers()[ckb_vm::registers::A3].to_u64();
                let k = get_arr(machine, k_addr, k_size)?;
                let v = get_arr(machine, v_addr, v_size)?;
                self.chain
                    .borrow_mut()
                    .set_storage(Bytes::from(k), Bytes::from(v))
                    .map_err(|e| ckb_vm::Error::InvalidEcall(code))?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            SYSCODE_GET_STORAGE => {
                let k_addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let k_size = machine.registers()[ckb_vm::registers::A1].to_u64();
                let v_addr = machine.registers()[ckb_vm::registers::A2].to_u64();
                let v_size = machine.registers()[ckb_vm::registers::A3].to_u64();
                let k = get_arr(machine, k_addr, k_size)?;
                let val = self
                    .chain
                    .borrow()
                    .get_storage(&Bytes::from(k))
                    .map_err(|e| ckb_vm::Error::InvalidEcall(code))?
                    .clone();
                machine.memory_mut().store_bytes(v_addr, &val)?;
                machine
                    .memory_mut()
                    .store_bytes(v_size, &(val.len() as u64).to_le_bytes())?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            SYSCODE_CONTRACT_CALL => {
                machine.add_cycles(CONTRACT_CALL_FIXED_CYCLE)?;
                let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let args_addr = machine.registers()[ckb_vm::registers::A1].to_u64();
                let args_size = machine.registers()[ckb_vm::registers::A2].to_u64();
                let ret_addr = machine.registers()[ckb_vm::registers::A3].to_u64();
                let ret_size = machine.registers()[ckb_vm::registers::A4].to_u64();
                let args = get_arr(machine, args_addr, args_size)?;
                let address_bytes = get_arr(machine, addr, 40)?;
                let address_hex = String::from_utf8_lossy(&address_bytes);
                let address = Address::from_hex(&address_hex).map_err(|e| {
                    ckb_vm::Error::EcallError(
                        SYSCODE_CONTRACT_CALL,
                        format!("invalid address: {}", address_hex),
                    )
                })?;
                let (ret, current_cycle) = self
                    .chain
                    .borrow_mut()
                    .contract_call(address, Bytes::from(args), machine.cycles())
                    .map_err(|e| {
                        ckb_vm::Error::EcallError(
                            SYSCODE_CONTRACT_CALL,
                            format!("contract call err: {}", e),
                        )
                    })?;
                machine.set_cycles(current_cycle);
                self.set_bytes(machine, ret_addr, ret_size, ret.as_bytes())?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
