//! Environmental Information
use ckb_vm::instructions::Register;
use ckb_vm::memory::Memory;
use protocol::{types::ServiceContext, Bytes};

use crate::vm::syscall::convention::{
    SYSCODE_ADDRESS, SYSCODE_BLOCK_HEIGHT, SYSCODE_CALLER, SYSCODE_CYCLE_LIMIT, SYSCODE_CYCLE_USED,
    SYSCODE_IS_INIT, SYSCODE_ORIGIN,
};
use crate::InterpreterParams;

pub struct SyscallEnvironment {
    context: ServiceContext,
    iparams: InterpreterParams,
}

impl SyscallEnvironment {
    pub fn new(context: ServiceContext, iparams: InterpreterParams) -> Self {
        Self { context, iparams }
    }
}

impl<Mac: ckb_vm::SupportMachine> ckb_vm::Syscalls<Mac> for SyscallEnvironment {
    fn initialize(&mut self, _machine: &mut Mac) -> Result<(), ckb_vm::Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, ckb_vm::Error> {
        let code = &machine.registers()[ckb_vm::registers::A7];
        match code.to_u64() {
            SYSCODE_ADDRESS => {
                let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                dbg!(self.iparams.address.as_hex());
                machine
                    .memory_mut()
                    .store_bytes(addr, self.iparams.address.as_hex().as_ref())?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            SYSCODE_CYCLE_LIMIT => {
                let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let gaslimit_byte = self.context.get_cycles_limit().to_le_bytes();
                machine.memory_mut().store_bytes(addr, &gaslimit_byte)?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            SYSCODE_CYCLE_USED => {
                let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let cycles_used = self.context.get_cycles_used().to_le_bytes();
                machine.memory_mut().store_bytes(addr, &cycles_used)?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            SYSCODE_IS_INIT => {
                let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let is_init: u64 = if self.iparams.is_init { 1 } else { 0 };
                machine
                    .memory_mut()
                    .store_bytes(addr, &is_init.to_le_bytes())?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            SYSCODE_ORIGIN => {
                let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                machine
                    .memory_mut()
                    .store_bytes(addr, self.context.get_caller().as_hex().as_ref())?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            SYSCODE_CALLER => {
                let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let caller = self
                    .context
                    .get_extra()
                    .unwrap_or_else(|| Bytes::from(self.context.get_caller().as_hex()));
                machine.memory_mut().store_bytes(addr, &caller)?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            SYSCODE_BLOCK_HEIGHT => {
                let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let block_height = self.context.get_current_height().to_le_bytes();
                machine.memory_mut().store_bytes(addr, &block_height)?;
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
                Ok(true)
            }
            // TODO: add system call to get other fields in context
            _ => Ok(false),
        }
    }
}
