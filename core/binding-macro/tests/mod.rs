#[macro_use]
extern crate core_binding_macro;

use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;
use json::JsonValue;

use protocol::traits::RequestContext;
use protocol::types::{Address, Hash};
use protocol::ProtocolResult;

#[test]
fn test_read_and_write() {
    struct Tests;

    impl Tests {
        #[read]
        fn test_read_fn<Context: RequestContext>(
            &self,
            _ctx: Context,
        ) -> ProtocolResult<JsonValue> {
            Ok(JsonValue::Null)
        }

        #[write]
        fn test_write_fn<Context: RequestContext>(
            &mut self,
            _ctx: Context,
        ) -> ProtocolResult<JsonValue> {
            Ok(JsonValue::Null)
        }
    }

    let context = MockRequestContext::new(1000);

    let mut t = Tests {};
    assert_eq!(t.test_read_fn(context.clone()).unwrap(), JsonValue::Null);
    assert_eq!(t.test_write_fn(context).unwrap(), JsonValue::Null);
}

#[test]
fn test_cycles() {
    struct Tests;

    impl Tests {
        #[cycles(100)]
        fn test_cycles<Context: RequestContext>(&self, ctx: Context) -> ProtocolResult<()> {
            Ok(())
        }

        #[cycles(500)]
        fn test_cycles2<Context: RequestContext>(&self, ctx: Context) -> ProtocolResult<()> {
            Ok(())
        }
    }

    #[cycles(200)]
    fn test_sub_cycles_fn1<Context: RequestContext>(ctx: Context) -> ProtocolResult<()> {
        Ok(())
    }

    #[cycles(200)]
    fn test_sub_cycles_fn2<Context: RequestContext>(_foo: u64, ctx: Context) -> ProtocolResult<()> {
        Ok(())
    }

    let t = Tests {};
    let context = MockRequestContext::new(1000);
    t.test_cycles(context.clone()).unwrap();
    assert_eq!(context.get_cycles_limit().unwrap(), 900);

    t.test_cycles2(context.clone()).unwrap();
    assert_eq!(context.get_cycles_limit().unwrap(), 400);

    test_sub_cycles_fn1(context.clone()).unwrap();
    assert_eq!(context.get_cycles_limit().unwrap(), 200);

    test_sub_cycles_fn2(1, context.clone()).unwrap();
    assert_eq!(context.get_cycles_limit().unwrap(), 0);
}

#[derive(Clone)]
struct MockRequestContext {
    cycles_limit: Rc<RefCell<u64>>,
}

impl MockRequestContext {
    pub fn new(cycles_limit: u64) -> Self {
        Self {
            cycles_limit: Rc::new(RefCell::new(cycles_limit)),
        }
    }
}

impl RequestContext for MockRequestContext {
    fn sub_cycles(&self, cycels: u64) -> ProtocolResult<()> {
        self.cycles_limit.replace_with(|&mut old| old - cycels);
        Ok(())
    }

    fn get_cycles_price(&self) -> ProtocolResult<u64> {
        Ok(0)
    }

    fn get_cycles_limit(&self) -> ProtocolResult<u64> {
        Ok(*self.cycles_limit.borrow())
    }

    fn get_cycles_used(&self) -> ProtocolResult<u64> {
        Ok(0)
    }

    fn get_caller(&self) -> ProtocolResult<Address> {
        Address::from_hash(Hash::digest(Bytes::from("test")))
    }

    fn get_current_epoch_id(&self) -> ProtocolResult<u64> {
        Ok(0)
    }

    fn get_service_name(&self) -> ProtocolResult<&str> {
        Ok("service")
    }

    fn get_service_method(&self) -> ProtocolResult<&str> {
        Ok("method")
    }

    fn get_payload(&self) -> ProtocolResult<&str> {
        Ok("payload")
    }
}
