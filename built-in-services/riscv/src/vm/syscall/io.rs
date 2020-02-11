// Since ckb-vm can only return 0 or 1 as exit code, We must find another way to
// return string, u64...
use std::cell::RefCell;
use std::{io, rc::Rc};

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
        let code = machine.registers()[ckb_vm::registers::A7].to_u64();

        match code {
            SYSCODE_RET => {
                let ptr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let size = machine.registers()[ckb_vm::registers::A1].to_u64();
                if ptr == 0 {
                    return Err(ckb_vm::Error::IO(io::ErrorKind::InvalidInput));
                }

                let buffer = get_arr(machine, ptr, size)?;
                self.output.borrow_mut().clear();
                self.output.borrow_mut().extend_from_slice(&buffer[..]);

                Ok(true)
            }
            SYSCODE_LOAD_ARGS => {
                let ptr = machine.registers()[ckb_vm::registers::A0].to_u64();

                if ptr != 0 {
                    machine.memory_mut().store_bytes(ptr, &self.input)?;
                }
                machine.set_register(
                    ckb_vm::registers::A0,
                    Mac::REG::from_u64(self.input.len() as u64),
                );

                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
