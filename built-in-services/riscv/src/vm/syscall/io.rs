// Since ckb-vm can only return 0 or 1 as exit code, We must find another way to
// return string, u64...
use std::cell::RefCell;
use std::rc::Rc;

use ckb_vm::instructions::Register;
use ckb_vm::Memory;

use crate::vm::syscall::common::get_arr;
use crate::vm::syscall::convention::{SYSCODE_LOAD_ARGS, SYSCODE_RET};

pub struct SyscallIO {
    input:  Vec<u8>,
    output: Rc<RefCell<Vec<u8>>>,
}

impl SyscallIO {
    pub fn new(input: Vec<u8>, output: Rc<RefCell<Vec<u8>>>) -> Self {
        Self { input, output }
    }
}

impl<Mac: ckb_vm::SupportMachine> ckb_vm::Syscalls<Mac> for SyscallIO {
    fn initialize(&mut self, _machine: &mut Mac) -> Result<(), ckb_vm::Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, ckb_vm::Error> {
        let code = &machine.registers()[ckb_vm::registers::A7];
        if code.to_u64() == SYSCODE_RET {
            let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
            let size = machine.registers()[ckb_vm::registers::A1].to_u64();
            let buffer = get_arr(machine, addr, size)?;
            self.output.borrow_mut().clear();
            self.output.borrow_mut().extend_from_slice(&buffer[..]);
            machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
            return Ok(true);
        }
        if code.to_u64() == SYSCODE_LOAD_ARGS {
            let addr = machine.registers()[ckb_vm::registers::A0].to_u64();
            machine.memory_mut().store_bytes(addr, &self.input)?;
            let len = machine.registers()[ckb_vm::registers::A1].to_u64();
            machine
                .memory_mut()
                .store_bytes(len, &(self.input.len() as u64).to_le_bytes())?;
            machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u8(0));
            return Ok(true);
        }
        Ok(false)
    }
}
