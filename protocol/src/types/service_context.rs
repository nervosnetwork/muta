use std::cell::RefCell;
use std::rc::Rc;

use derive_more::{Display, From};

use crate::types::{Address, Event, Hash};
use crate::{ProtocolError, ProtocolErrorKind, ProtocolResult};

#[derive(Debug, Clone)]
pub struct ServiceContextParams {
    pub tx_hash:         Option<Hash>,
    pub nonce:           Option<Hash>,
    pub cycles_limit:    u64,
    pub cycles_price:    u64,
    pub cycles_used:     Rc<RefCell<u64>>,
    pub caller:          Address,
    pub epoch_id:        u64,
    pub service_name:    String,
    pub service_method:  String,
    pub service_payload: String,
    pub timestamp:       u64,
    pub events:          Rc<RefCell<Vec<Event>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceContext {
    tx_hash:         Option<Hash>,
    nonce:           Option<Hash>,
    cycles_limit:    u64,
    cycles_price:    u64,
    cycles_used:     Rc<RefCell<u64>>,
    caller:          Address,
    epoch_id:        u64,
    service_name:    String,
    service_method:  String,
    service_payload: String,
    timestamp:       u64,
    events:          Rc<RefCell<Vec<Event>>>,
}

impl ServiceContext {
    pub fn new(params: ServiceContextParams) -> Self {
        Self {
            tx_hash:         params.tx_hash,
            nonce:           params.nonce,
            cycles_limit:    params.cycles_limit,
            cycles_price:    params.cycles_price,
            cycles_used:     params.cycles_used,
            caller:          params.caller,
            epoch_id:        params.epoch_id,
            service_name:    params.service_name,
            service_method:  params.service_method,
            service_payload: params.service_payload,
            timestamp:       params.timestamp,
            events:          params.events,
        }
    }

    pub fn with_context(
        context: &ServiceContext,
        service_name: String,
        service_method: String,
        service_payload: String,
    ) -> Self {
        Self {
            tx_hash: context.tx_hash.clone(),
            nonce: context.nonce.clone(),
            cycles_limit: context.cycles_limit,
            cycles_price: context.cycles_price,
            cycles_used: Rc::clone(&context.cycles_used),
            caller: context.caller.clone(),
            epoch_id: context.epoch_id,
            service_name,
            service_method,
            service_payload,
            timestamp: context.get_timestamp(),
            events: Rc::clone(&context.events),
        }
    }

    pub fn get_tx_hash(&self) -> Option<Hash> {
        self.tx_hash.clone()
    }

    pub fn get_nonce(&self) -> Option<Hash> {
        self.nonce.clone()
    }

    pub fn get_events(&self) -> Vec<Event> {
        self.events.borrow().clone()
    }

    pub fn sub_cycles(&self, cycles: u64) -> ProtocolResult<()> {
        if self.get_cycles_used() + cycles <= self.cycles_limit {
            *self.cycles_used.borrow_mut() = self.get_cycles_used() + cycles;
            Ok(())
        } else {
            Err(ServiceContextError::OutOfCycles.into())
        }
    }

    pub fn get_cycles_price(&self) -> u64 {
        self.cycles_price
    }

    pub fn get_cycles_limit(&self) -> u64 {
        self.cycles_limit
    }

    pub fn get_cycles_used(&self) -> u64 {
        *self.cycles_used.borrow()
    }

    pub fn get_caller(&self) -> Address {
        self.caller.clone()
    }

    pub fn get_current_epoch_id(&self) -> u64 {
        self.epoch_id
    }

    pub fn get_service_name(&self) -> &str {
        &self.service_name
    }

    pub fn get_service_method(&self) -> &str {
        &self.service_method
    }

    pub fn get_payload(&self) -> &str {
        &self.service_payload
    }

    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn emit_event(&self, message: String) -> ProtocolResult<()> {
        self.events.borrow_mut().push(Event {
            service: self.service_name.clone(),
            data:    message,
        });

        Ok(())
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceContextError {
    #[display(fmt = "out of cycles")]
    OutOfCycles,
}

impl std::error::Error for ServiceContextError {}

impl From<ServiceContextError> for ProtocolError {
    fn from(err: ServiceContextError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{ServiceContext, ServiceContextParams};
    use crate::types::{Address, Hash};

    #[test]
    fn test_request_context() {
        let params = ServiceContextParams {
            tx_hash:         None,
            nonce:           None,
            cycles_limit:    100,
            cycles_price:    8,
            cycles_used:     Rc::new(RefCell::new(10)),
            caller:          Address::from_hash(Hash::from_empty()).unwrap(),
            epoch_id:        1,
            timestamp:       0,
            service_name:    "service_name".to_owned(),
            service_method:  "service_method".to_owned(),
            service_payload: "service_payload".to_owned(),
            events:          Rc::new(RefCell::new(vec![])),
        };
        let ctx = ServiceContext::new(params);

        ctx.sub_cycles(8).unwrap();
        assert_eq!(ctx.get_cycles_used(), 18);

        assert_eq!(ctx.get_cycles_limit(), 100);
        assert_eq!(ctx.get_cycles_price(), 8);
        assert_eq!(
            ctx.get_caller(),
            Address::from_hash(Hash::from_empty()).unwrap()
        );
        assert_eq!(ctx.get_current_epoch_id(), 1);
        assert_eq!(ctx.get_timestamp(), 0);
        assert_eq!(ctx.get_service_name(), "service_name");
        assert_eq!(ctx.get_service_method(), "service_method");
        assert_eq!(ctx.get_payload(), "service_payload");
    }
}
