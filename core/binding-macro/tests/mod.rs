#[macro_use]
extern crate core_binding_macro;

use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use core_binding_macro::{cycles, read, write};
use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    RequestContext, Service, ServiceSDK, StoreArray, StoreBool, StoreMap, StoreString, StoreUint64,
};
use protocol::types::{Address, Epoch, Hash, Receipt, SignedTransaction};
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
    struct Tests<SDK: ServiceSDK> {
        _sdk:        SDK,
        hook_before: bool,
        hook_after:  bool,
    }

    #[service]
    impl<SDK: ServiceSDK> Tests<SDK> {
        #[init]
        fn custom_init(_sdk: SDK) -> ProtocolResult<Self> {
            Ok(Self {
                _sdk,
                hook_after: false,
                hook_before: false,
            })
        }

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

    let sdk = MockServiceSDK {};
    let mut test_service = Tests::init_(sdk).unwrap();

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

    fn emit_event(&mut self, _message: String) -> ProtocolResult<()> {
        unimplemented!()
    }
}

struct MockServiceSDK;

impl ServiceSDK for MockServiceSDK {
    type ContextItem = MockRequestContext;

    // Alloc or recover a `Map` by` var_name`
    fn alloc_or_recover_map<Key: 'static + FixedCodec + PartialEq, Val: 'static + FixedCodec>(
        &mut self,
        _var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreMap<Key, Val>>> {
        unimplemented!()
    }

    // Alloc or recover a `Array` by` var_name`
    fn alloc_or_recover_array<Elm: 'static + FixedCodec>(
        &mut self,
        _var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreArray<Elm>>> {
        unimplemented!()
    }

    // Alloc or recover a `Uint64` by` var_name`
    fn alloc_or_recover_uint64(&mut self, _var_name: &str) -> ProtocolResult<Box<dyn StoreUint64>> {
        unimplemented!()
    }

    // Alloc or recover a `String` by` var_name`
    fn alloc_or_recover_string(&mut self, _var_name: &str) -> ProtocolResult<Box<dyn StoreString>> {
        unimplemented!()
    }

    // Alloc or recover a `Bool` by` var_name`
    fn alloc_or_recover_bool(&mut self, _var_name: &str) -> ProtocolResult<Box<dyn StoreBool>> {
        unimplemented!()
    }

    // Get a value from the service state by key
    fn get_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        _key: &Key,
    ) -> ProtocolResult<Option<Ret>> {
        unimplemented!()
    }

    // Set a value to the service state by key
    fn set_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        _key: Key,
        _val: Val,
    ) -> ProtocolResult<()> {
        unimplemented!()
    }

    // Get a value from the specified address by key
    fn get_account_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        _address: &Address,
        _key: &Key,
    ) -> ProtocolResult<Option<Ret>> {
        unimplemented!()
    }

    // Insert a pair of key / value to the specified address
    fn set_account_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        _address: &Address,
        _key: Key,
        _val: Val,
    ) -> ProtocolResult<()> {
        unimplemented!()
    }

    // Get a signed transaction by `tx_hash`
    // if not found on the chain, return None
    fn get_transaction_by_hash(
        &self,
        _tx_hash: &Hash,
    ) -> ProtocolResult<Option<SignedTransaction>> {
        unimplemented!()
    }

    // Get a epoch by `epoch_id`
    // if not found on the chain, return None
    // When the parameter `epoch_id` is None, get the latest (executing)` epoch`
    fn get_epoch_by_epoch_id(&self, _epoch_id: Option<u64>) -> ProtocolResult<Option<Epoch>> {
        unimplemented!()
    }

    // Get a receipt by `tx_hash`
    // if not found on the chain, return None
    fn get_receipt_by_hash(&self, _tx_hash: &Hash) -> ProtocolResult<Option<Receipt>> {
        unimplemented!()
    }

    fn get_request_context(&self) -> ProtocolResult<Self::ContextItem> {
        unimplemented!()
    }

    // Call other read-only methods of `service` and return the results
    // synchronously NOTE: You can use recursive calls, but the maximum call
    // stack is 1024
    fn read(&self, _service: &str, _method: &str, _payload: &str) -> ProtocolResult<&str> {
        unimplemented!()
    }

    // Call other writable methods of `service` and return the results synchronously
    // NOTE: You can use recursive calls, but the maximum call stack is 1024
    fn write(&mut self, _service: &str, _method: &str, _payload: &str) -> ProtocolResult<&str> {
        unimplemented!()
    }
}
