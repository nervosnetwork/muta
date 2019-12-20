#[macro_use]
extern crate core_binding_macro;

use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use core_binding_macro::{cycles, read, write};
use protocol::traits::{RequestContext, Service};
use protocol::types::{Address, Hash};
use protocol::ProtocolResult;

#[test]
fn test_read_and_write() {
    struct Tests;

    impl Tests {
        #[read]
        fn test_read_fn<Context: RequestContext>(&self, _ctx: Context) -> ProtocolResult<String> {
            Ok("read".to_owned())
        }

        #[write]
        fn test_write_fn<Context: RequestContext>(
            &mut self,
            _ctx: Context,
        ) -> ProtocolResult<String> {
            Ok("write".to_owned())
        }
    }

    let context = MockRequestContext::new(1000);

    let mut t = Tests {};
    assert_eq!(t.test_read_fn(context.clone()).unwrap(), "read".to_owned());
    assert_eq!(t.test_write_fn(context).unwrap(), "write".to_owned());
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
    assert_eq!(context.get_cycles_limit(), 900);

    t.test_cycles2(context.clone()).unwrap();
    assert_eq!(context.get_cycles_limit(), 400);

    test_sub_cycles_fn1(context.clone()).unwrap();
    assert_eq!(context.get_cycles_limit(), 200);

    test_sub_cycles_fn2(1, context.clone()).unwrap();
    assert_eq!(context.get_cycles_limit(), 0);
}

#[test]
fn test_impl_service() {
    #[derive(Serialize, Deserialize, Debug)]
    struct TestServicePayload {
        name: String,
        age:  u64,
        sex:  bool,
    }
    struct Tests {
        hook_before: bool,
        hook_after:  bool,
    }

    #[service]
    impl Tests {
        #[hook_before]
        fn custom_hook_before(&mut self) -> ProtocolResult<()> {
            self.hook_before = true;
            Ok(())
        }

        #[hook_after]
        fn custom_hook_after(&mut self) -> ProtocolResult<()> {
            self.hook_after = true;
            Ok(())
        }

        #[read]
        fn test_read<Context: RequestContext>(
            &self,
            _ctx: Context,
            _payload: TestServicePayload,
        ) -> ProtocolResult<String> {
            Ok("read ok".to_owned())
        }

        #[write]
        fn test_write<Context: RequestContext>(
            &mut self,
            _ctx: Context,
            _payload: TestServicePayload,
        ) -> ProtocolResult<String> {
            Ok("write ok".to_owned())
        }
    }

    let payload = TestServicePayload {
        name: "test".to_owned(),
        age:  10,
        sex:  false,
    };
    let payload_str = serde_json::to_string(&payload).unwrap();

    let mut test_service = Tests {
        hook_before: false,
        hook_after:  false,
    };
    let context = MockRequestContext::with_method(1024 * 1024, "test_write", &payload_str);
    let write_res = test_service.write_(context).unwrap();
    assert_eq!(write_res, "write ok");

    let context = MockRequestContext::with_method(1024 * 1024, "test_read", &payload_str);
    let read_res = test_service.read_(context).unwrap();
    assert_eq!(read_res, "read ok");

    let context = MockRequestContext::with_method(1024 * 1024, "test_notfound", &payload_str);
    let read_res = test_service.read_(context.clone());
    assert_eq!(read_res.is_err(), true);
    let write_res = test_service.write_(context);
    assert_eq!(write_res.is_err(), true);

    test_service.hook_before_().unwrap();
    assert_eq!(test_service.hook_before, true);

    test_service.hook_after_().unwrap();
    assert_eq!(test_service.hook_after, true);
}

#[derive(Clone)]
struct MockRequestContext {
    cycles_limit: Rc<RefCell<u64>>,
    method:       String,
    payload:      String,
}

impl MockRequestContext {
    pub fn new(cycles_limit: u64) -> Self {
        Self {
            cycles_limit: Rc::new(RefCell::new(cycles_limit)),
            method:       "method".to_owned(),
            payload:      "payload".to_owned(),
        }
    }

    pub fn with_method(cycles_limit: u64, method: &str, payload: &str) -> Self {
        Self {
            cycles_limit: Rc::new(RefCell::new(cycles_limit)),
            method:       method.to_owned(),
            payload:      payload.to_owned(),
        }
    }
}

impl RequestContext for MockRequestContext {
    fn sub_cycles(&self, cycles: u64) -> ProtocolResult<()> {
        self.cycles_limit.replace_with(|&mut old| old - cycles);
        Ok(())
    }

    fn get_cycles_price(&self) -> u64 {
        0
    }

    fn get_cycles_limit(&self) -> u64 {
        *self.cycles_limit.borrow()
    }

    fn get_cycles_used(&self) -> u64 {
        0
    }

    fn get_caller(&self) -> Address {
        Address::from_hash(Hash::digest(Bytes::from("test"))).unwrap()
    }

    fn get_current_epoch_id(&self) -> u64 {
        0
    }

    fn get_service_name(&self) -> &str {
        "service"
    }

    fn get_service_method(&self) -> &str {
        &self.method
    }

    fn get_payload(&self) -> &str {
        &self.payload
    }

    fn emit_event(&mut self, message: String) -> ProtocolResult<()> {
        unimplemented!()
    }
}
