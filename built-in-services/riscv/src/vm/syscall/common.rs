use std::io::ErrorKind;

use ckb_vm::instructions::Register;
use ckb_vm::Memory;

// Get a string from memory, stop with '\0' flag.
pub fn get_str<Mac: ckb_vm::SupportMachine>(
    machine: &mut Mac,
    addr: u64,
) -> Result<String, ckb_vm::Error> {
    let mut addr = addr;
    let mut buffer = Vec::new();

    loop {
        let byte = machine
            .memory_mut()
            .load8(&Mac::REG::from_u64(addr))?
            .to_u8();
        if byte == 0 {
            break;
        }
        buffer.push(byte);
        addr += 1;
    }

    machine.add_cycles(buffer.len() as u64 * 10)?;
    Ok(String::from_utf8(buffer).map_err(|_| ckb_vm::Error::IO(ErrorKind::InvalidData))?)
}

// Get a byte array from memory by exact size
pub fn get_arr<Mac: ckb_vm::SupportMachine>(
    machine: &mut Mac,
    addr: u64,
    size: u64,
) -> Result<Vec<u8>, ckb_vm::Error> {
    let mut addr = addr;
    let mut buffer = Vec::new();
    for _ in 0..size {
        let byte = machine
            .memory_mut()
            .load8(&Mac::REG::from_u64(addr))?
            .to_u8();
        buffer.push(byte);
        addr += 1;
    }
    machine.add_cycles(buffer.len() as u64 * 10)?;
    Ok(buffer)
}
