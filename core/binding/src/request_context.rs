use std::cell::RefCell;
use std::rc::Rc;

use derive_more::{Display, From};

use protocol::traits::RequestContext;
use protocol::types::{Address, Event};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

#[derive(Debug, Clone, PartialEq)]
pub struct DefaultRequestContext {
    cycles_limit:     u64,
    cycles_price:     u64,
    cycles_used:      Rc<RefCell<u64>>,
    caller:           Address,
    epoch_id:         u64,
    service_name:     String,
    service_method:   String,
    service_playload: String,
    events:           Rc<RefCell<Vec<Event>>>,
}

impl DefaultRequestContext {
    pub fn new(
        cycles_limit: u64,
        cycles_price: u64,
        cycles_used: u64,
        caller: Address,
        epoch_id: u64,
        service_name: String,
        service_method: String,
        service_playload: String,
    ) -> Self {
        Self {
            cycles_limit,
            cycles_price,
            cycles_used: Rc::new(RefCell::new(cycles_used)),
            caller,
            epoch_id,
            service_name,
            service_method,
            service_playload,
            events: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn derive_context(
        context: &DefaultRequestContext,
        service_name: String,
        service_method: String,
        service_playload: String,
    ) -> Self {
        Self {
            cycles_limit: context.cycles_limit,
            cycles_price: context.cycles_price,
            cycles_used: Rc::clone(&context.cycles_used),
            caller: context.caller.clone(),
            epoch_id: context.epoch_id,
            service_name,
            service_method,
            service_playload,
            events: Rc::clone(&context.events),
        }
    }
}

impl RequestContext for DefaultRequestContext {
    fn sub_cycles(&mut self, cycles: u64) -> ProtocolResult<()> {
        if self.get_cycles_used() + cycles <= self.cycles_limit {
            *self.cycles_used.borrow_mut() = self.get_cycles_used() + cycles;
            Ok(())
        } else {
            Err(ContextError::OutOfCycles.into())
        }
    }

    fn get_cycles_price(&self) -> u64 {
        self.cycles_price
    }

    fn get_cycles_limit(&self) -> u64 {
        self.cycles_limit
    }

    fn get_cycles_used(&self) -> u64 {
        *self.cycles_used.borrow()
    }

    fn get_caller(&self) -> Address {
        self.caller.clone()
    }

    fn get_current_epoch_id(&self) -> u64 {
        self.epoch_id
    }

    fn get_service_name(&self) -> &str {
        &self.service_name
    }

    fn get_service_method(&self) -> &str {
        &self.service_method
    }

    fn get_playload(&self) -> &str {
        &self.service_playload
    }

    fn emit_event(&mut self, message: String) -> ProtocolResult<()> {
        self.events.borrow_mut().push(Event {
            service: self.service_name.clone(),
            data:    message,
        });

        Ok(())
    }
}

#[derive(Debug, Display, From)]
pub enum ContextError {
    #[display(fmt = "out of cycles")]
    OutOfCycles,
}

impl std::error::Error for ContextError {}

impl From<ContextError> for ProtocolError {
    fn from(err: ContextError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Binding, Box::new(err))
    }
}
