//! Environmental Information
use std::cell::RefCell;
use std::{io, rc::Rc};

use ckb_vm::instructions::Register;
use ckb_vm::memory::Memory;
use protocol::{types::Address, Bytes};

use crate::vm::cost_model::CONTRACT_CALL_FIXED_CYCLE;
use crate::vm::syscall::common::{get_arr, get_str};
use crate::vm::syscall::convention::{
    SYSCODE_CONTRACT_CALL, SYSCODE_GET_STORAGE, SYSCODE_SERVICE_CALL, SYSCODE_SET_STORAGE,
};
use crate::ChainInterface;

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
        use ckb_vm::Error::*;
        use std::io::ErrorKind::*;

        let code = machine.registers()[ckb_vm::registers::A7].to_u64();

        match code {
            SYSCODE_SET_STORAGE => {
                let key_ptr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let key_len = machine.registers()[ckb_vm::registers::A1].to_u64();
                let val_ptr = machine.registers()[ckb_vm::registers::A2].to_u64();
                let val_len = machine.registers()[ckb_vm::registers::A3].to_u64();
                if key_ptr == 0 || val_ptr == 0 || key_len == 0 {
                    return Err(ckb_vm::Error::IO(io::ErrorKind::InvalidInput));
                }

                let key = get_arr(machine, key_ptr, key_len)?;
                let val = get_arr(machine, val_ptr, val_len)?;

                self.chain
                    .borrow_mut()
                    .set_storage(Bytes::from(key), Bytes::from(val))
                    .map_err(|_| ckb_vm::Error::IO(io::ErrorKind::Other))?;

                Ok(true)
            }
            SYSCODE_GET_STORAGE => {
                let key_ptr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let key_len = machine.registers()[ckb_vm::registers::A1].to_u64();
                let val_ptr = machine.registers()[ckb_vm::registers::A2].to_u64();
                if key_ptr == 0 || key_len == 0 {
                    return Err(ckb_vm::Error::IO(io::ErrorKind::InvalidInput));
                }

                let key = get_arr(machine, key_ptr, key_len)?;
                let val = self
                    .chain
                    .borrow()
                    .get_storage(&Bytes::from(key))
                    .map_err(|_| ckb_vm::Error::IO(io::ErrorKind::Other))?;

                if val_ptr != 0 {
                    machine.memory_mut().store_bytes(val_ptr, &val)?;
                }
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u64(val.len() as u64));

                Ok(true)
            }
            SYSCODE_CONTRACT_CALL => {
                machine.add_cycles(CONTRACT_CALL_FIXED_CYCLE)?;

                let addr_ptr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let args_ptr = machine.registers()[ckb_vm::registers::A1].to_u64();
                let args_len = machine.registers()[ckb_vm::registers::A2].to_u64();
                let ret_ptr = machine.registers()[ckb_vm::registers::A3].to_u64();

                if addr_ptr == 0 {
                    return Err(ckb_vm::Error::IO(io::ErrorKind::InvalidInput));
                }

                let call_args = if args_ptr != 0 {
                    Bytes::from(get_arr(machine, args_ptr, args_len)?)
                } else {
                    Bytes::new()
                };

                let address = {
                    let hex = String::from_utf8(get_arr(machine, addr_ptr, 40)?)
                        .map_err(|_| IO(InvalidData))?;
                    Address::from_hex(&hex).map_err(|_| IO(InvalidData))?
                };

                let (ret, current_cycle) = self
                    .chain
                    .borrow_mut()
                    .contract_call(address, call_args, machine.cycles())
                    .map_err(|_| ckb_vm::Error::IO(io::ErrorKind::Other))?;

                machine.set_cycles(current_cycle);
                if ret_ptr != 0 {
                    machine.memory_mut().store_bytes(ret_ptr, ret.as_ref())?;
                }
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u64(ret.len() as u64));

                Ok(true)
            }
            SYSCODE_SERVICE_CALL => {
                machine.add_cycles(CONTRACT_CALL_FIXED_CYCLE)?;

                let service_ptr = machine.registers()[ckb_vm::registers::A0].to_u64();
                let method_ptr = machine.registers()[ckb_vm::registers::A1].to_u64();
                let payload_ptr = machine.registers()[ckb_vm::registers::A2].to_u64();
                let payload_len = machine.registers()[ckb_vm::registers::A3].to_u64();
                let ret_ptr = machine.registers()[ckb_vm::registers::A4].to_u64();
                if service_ptr == 0 || method_ptr == 0 {
                    return Err(ckb_vm::Error::IO(io::ErrorKind::InvalidInput));
                }

                let service = get_str(machine, service_ptr)?;
                let method = get_str(machine, method_ptr)?;

                // FIXME: Right now, service call payload is json, but this may
                // change. May become bytes. Use from_utf8_lossy here so we're
                // not force json.
                let payload = if payload_ptr != 0 {
                    get_arr(machine, payload_ptr, payload_len)?
                } else {
                    Vec::new()
                };
                let payload = String::from_utf8_lossy(&payload);

                let (ret, current_cycle) = self
                    .chain
                    .borrow_mut()
                    .service_call(&service, &method, &payload, machine.cycles())
                    .map_err(|_| ckb_vm::Error::IO(io::ErrorKind::Other))?;

                machine.set_cycles(current_cycle);
                if ret_ptr != 0 {
                    machine.memory_mut().store_bytes(ret_ptr, ret.as_ref())?;
                }
                machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u64(ret.len() as u64));

                Ok(true)
            }

            _ => Ok(false),
        }
    }
}
