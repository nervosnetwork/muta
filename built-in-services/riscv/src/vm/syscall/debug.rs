//! Provedis a debug function, let the contract print information to standard
//! output.
use std::io::Write;

use ckb_vm::instructions::Register;

use crate::vm::syscall::common::get_str;
use crate::vm::syscall::convention::SYSCODE_DEBUG;

pub struct SyscallDebug<T> {
    prefix: &'static str,
    output: T,
}

impl<T: Write> SyscallDebug<T> {
    pub fn new(prefix: &'static str, output: T) -> Self {
        Self { prefix, output }
    }
}

impl<Mac: ckb_vm::SupportMachine, T: Write> ckb_vm::Syscalls<Mac> for SyscallDebug<T> {
    fn initialize(&mut self, _machine: &mut Mac) -> Result<(), ckb_vm::Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, ckb_vm::Error> {
        let code = machine.registers()[ckb_vm::registers::A7].to_u64();
        if code != SYSCODE_DEBUG {
            return Ok(false);
        }

        let ptr = machine.registers()[ckb_vm::registers::A0].to_u64();
        if ptr == 0 {
            return Err(ckb_vm::Error::IO(std::io::ErrorKind::InvalidInput));
        }

        let msg = get_str(machine, ptr)?;
        self.output
            .write_fmt(format_args!("{} [{}]\n", self.prefix, msg))?;

        Ok(true)
    }
}
