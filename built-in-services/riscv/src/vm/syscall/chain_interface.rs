//! Environmental Information
use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;
use ckb_vm::instructions::Register;
use ckb_vm::memory::Memory;
use protocol::types::{Address, Hash, ServiceContext};

use crate::vm::syscall::common::get_arr;
use crate::vm::syscall::convention::{SYSCODE_GET_STORAGE, SYSCODE_SET_STORAGE};
use crate::ChainInterface;
use crate::InterpreterParams;

pub struct SyscallChainInterface {
    chain: Rc<RefCell<dyn ChainInterface>>,
}

impl SyscallChainInterface {
    pub fn new(chain: Rc<RefCell<dyn ChainInterface>>) -> Self {
        Self { chain }
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
            _ => Ok(false),
        }
    }
}
