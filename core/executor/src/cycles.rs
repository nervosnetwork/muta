use std::collections::HashMap;
use std::error::Error;

use derive_more::{Display, From};
use lazy_static::lazy_static;

use protocol::types::Fee;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

const NATIVE_BASE_CYCLES: u64 = 10;

lazy_static! {
    static ref CYCLES_TABLE: HashMap<CyclesAction, u64> = {
        let mut table = HashMap::new();
        table.insert(CyclesAction::AccountTransfer, NATIVE_BASE_CYCLES * 21);
        table.insert(CyclesAction::BankRegister, NATIVE_BASE_CYCLES * 210);
        table
    };
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CyclesAction {
    AccountTransfer,
    BankRegister,
}

pub fn consume_cycles(
    action: CyclesAction,
    cycles_price: u64,
    fee: &mut Fee,
    limit: &Fee,
) -> ProtocolResult<()> {
    let cycles_used = fee.cycle
        + CYCLES_TABLE
            .get(&action)
            .unwrap_or_else(|| panic!("cycles action {:?} uninitialized", action));
    let cycles_used = cycles_used * cycles_price;

    if cycles_used > limit.cycle {
        return Err(CyclesError::OutOfCycles.into());
    }

    fee.cycle = cycles_used;
    Ok(())
}

#[derive(Debug, Display, From)]
pub enum CyclesError {
    #[display(fmt = "out of cycles")]
    OutOfCycles,
}

impl Error for CyclesError {}

impl From<CyclesError> for ProtocolError {
    fn from(err: CyclesError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
