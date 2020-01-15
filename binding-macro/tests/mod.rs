#![allow(clippy::unit_cmp)]
#[macro_use]
extern crate binding_macro;

use std::cell::RefCell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    ExecutorParams, Service, ServiceSDK, StoreArray, StoreBool, StoreMap, StoreString, StoreUint64,
};
use protocol::types::{
    Address, Epoch, Hash, Receipt, ServiceContext, ServiceContextParams, SignedTransaction,
};
use protocol::ProtocolResult;

#[test]
fn test_read_and_write() {
    struct Tests;

    impl Tests {
        #[read]
        fn test_read_fn(&self, _ctx: ServiceContext, _s: &str) -> ProtocolResult<String> {
            Ok("read".to_owned())
        }

        #[write]
        fn test_write_fn(&mut self, _ctx: ServiceContext, _s: &str) -> ProtocolResult<String> {
            Ok("write".to_owned())
        }
    }

    let context = get_context(1000, "", "", "");

    let mut t = Tests {};
    assert_eq!(
        t.test_read_fn(context.clone(), "read").unwrap(),
        "read".to_owned()
    );
    assert_eq!(
        t.test_write_fn(context, "write").unwrap(),
        "write".to_owned()
    );
}

#[test]
fn test_hooks() {
    struct Tests {
        pub epoch_id: u64,
    };

    impl Tests {
        #[hook_after]
        fn hook_after(&mut self, params: &ExecutorParams) -> ProtocolResult<()> {
            self.epoch_id = params.epoch_id;
            Ok(())
        }

        #[hook_before]
        fn hook_before(&mut self, params: &ExecutorParams) -> ProtocolResult<()> {
            self.epoch_id = params.epoch_id;
            Ok(())
        }
    }

    let mut t = Tests { epoch_id: 0 };
    t.hook_after(&mock_executor_params()).unwrap();
    assert_eq!(t.epoch_id, 9);
    t.hook_before(&mock_executor_params()).unwrap();
    assert_eq!(t.epoch_id, 9);
}

#[test]
fn test_read_and_write_with_noneparams() {
    struct Tests;

    impl Tests {
        #[read]
        fn test_read_fn(&self, _ctx: ServiceContext) -> ProtocolResult<()> {
            Ok(())
        }

        #[write]
        fn test_write_fn(&mut self, _ctx: ServiceContext) -> ProtocolResult<()> {
            Ok(())
        }
    }

    let context = get_context(1000, "", "", "");

    let mut t = Tests {};
    assert_eq!(t.test_read_fn(context.clone()).unwrap(), ());
    assert_eq!(t.test_write_fn(context).unwrap(), ());
}

#[test]
fn test_cycles() {
    struct Tests;

    impl Tests {
        #[cycles(100)]
        fn test_cycles(&self, ctx: ServiceContext) -> ProtocolResult<()> {
            Ok(())
        }

        #[cycles(500)]
        fn test_cycles2(&self, ctx: ServiceContext) -> ProtocolResult<()> {
            Ok(())
        }
    }

    #[cycles(200)]
    fn test_sub_cycles_fn1(ctx: ServiceContext) -> ProtocolResult<()> {
        Ok(())
    }

    #[cycles(200)]
    fn test_sub_cycles_fn2(_foo: u64, ctx: ServiceContext) -> ProtocolResult<()> {
        Ok(())
    }

    let t = Tests {};
    let context = get_context(1000, "", "", "");
    t.test_cycles(context.clone()).unwrap();
    assert_eq!(context.get_cycles_used(), 100);

    t.test_cycles2(context.clone()).unwrap();
    assert_eq!(context.get_cycles_used(), 600);

    test_sub_cycles_fn1(context.clone()).unwrap();
    assert_eq!(context.get_cycles_used(), 800);

    test_sub_cycles_fn2(1, context.clone()).unwrap();
    assert_eq!(context.get_cycles_used(), 1000);
}

#[test]
fn test_service() {
    #[derive(Serialize, Deserialize, Debug)]
    struct TestServicePayload {
        name: String,
        age:  u64,
        sex:  bool,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct TestServiceResponse {
        pub message: String,
    }

    struct Tests<SDK: ServiceSDK> {
        _sdk:         SDK,
        genesis_data: String,
        hook_before:  bool,
        hook_after:   bool,
    }

    #[service]
    impl<SDK: ServiceSDK> Tests<SDK> {
        #[genesis]
        fn init_genesis(&mut self) -> ProtocolResult<()> {
            self.genesis_data = "genesis".to_owned();

            Ok(())
        }

        #[hook_before]
        fn custom_hook_before(&mut self, _params: &ExecutorParams) -> ProtocolResult<()> {
            self.hook_before = true;
            Ok(())
        }

        #[hook_after]
        fn custom_hook_after(&mut self, _params: &ExecutorParams) -> ProtocolResult<()> {
            self.hook_after = true;
            Ok(())
        }

        #[read]
        fn test_read(
            &self,
            _ctx: ServiceContext,
            _payload: TestServicePayload,
        ) -> ProtocolResult<TestServiceResponse> {
            Ok(TestServiceResponse {
                message: "read ok".to_owned(),
            })
        }

        #[write]
        fn test_write(
            &mut self,
            _ctx: ServiceContext,
            _payload: TestServicePayload,
        ) -> ProtocolResult<TestServiceResponse> {
            Ok(TestServiceResponse {
                message: "write ok".to_owned(),
            })
        }
    }

    let payload = TestServicePayload {
        name: "test".to_owned(),
        age:  10,
        sex:  false,
    };
    let payload_str = serde_json::to_string(&payload).unwrap();

    let sdk = MockServiceSDK {};
    let mut test_service = Tests {
        _sdk:         sdk,
        genesis_data: "".to_owned(),
        hook_after:   false,
        hook_before:  false,
    };

    test_service.genesis_("".to_owned()).unwrap();
    assert_eq!(test_service.genesis_data, "genesis");

    let context = get_context(1024 * 1024, "", "test_write", &payload_str);
    let write_res = test_service.write_(context).unwrap();
    assert_eq!(write_res, r#"{"message":"write ok"}"#);

    let context = get_context(1024 * 1024, "", "test_read", &payload_str);
    let read_res = test_service.read_(context).unwrap();
    assert_eq!(read_res, r#"{"message":"read ok"}"#);

    let context = get_context(1024 * 1024, "", "test_notfound", &payload_str);
    let read_res = test_service.read_(context.clone());
    assert_eq!(read_res.is_err(), true);
    let write_res = test_service.write_(context);
    assert_eq!(write_res.is_err(), true);

    test_service.hook_before_(&mock_executor_params()).unwrap();
    assert_eq!(test_service.hook_before, true);

    test_service.hook_after_(&mock_executor_params()).unwrap();
    assert_eq!(test_service.hook_after, true);
}

#[test]
fn test_service_none_payload() {
    #[derive(Serialize, Deserialize, Debug)]
    struct TestServiceResponse {
        pub message: String,
    }

    struct Tests<SDK: ServiceSDK> {
        _sdk:         SDK,
        genesis_data: String,
        hook_before:  bool,
        hook_after:   bool,
    }

    #[service]
    impl<SDK: ServiceSDK> Tests<SDK> {
        #[genesis]
        fn init_genesis(&mut self) -> ProtocolResult<()> {
            self.genesis_data = "genesis".to_owned();

            Ok(())
        }

        #[hook_before]
        fn custom_hook_before(&mut self, _params: &ExecutorParams) -> ProtocolResult<()> {
            self.hook_before = true;
            Ok(())
        }

        #[hook_after]
        fn custom_hook_after(&mut self, _params: &ExecutorParams) -> ProtocolResult<()> {
            self.hook_after = true;
            Ok(())
        }

        #[read]
        fn test_read(&self, _ctx: ServiceContext) -> ProtocolResult<TestServiceResponse> {
            Ok(TestServiceResponse {
                message: "read ok".to_owned(),
            })
        }

        #[write]
        fn test_write(&mut self, _ctx: ServiceContext) -> ProtocolResult<TestServiceResponse> {
            Ok(TestServiceResponse {
                message: "write ok".to_owned(),
            })
        }
    }

    let sdk = MockServiceSDK {};
    let mut test_service = Tests {
        _sdk:         sdk,
        genesis_data: "".to_owned(),
        hook_after:   false,
        hook_before:  false,
    };

    test_service.genesis_("".to_owned()).unwrap();
    assert_eq!(test_service.genesis_data, "genesis");

    let context = get_context(1024 * 1024, "", "test_write", "");
    let write_res = test_service.write_(context).unwrap();
    assert_eq!(write_res, r#"{"message":"write ok"}"#);

    let context = get_context(1024 * 1024, "", "test_read", "");
    let read_res = test_service.read_(context).unwrap();
    assert_eq!(read_res, r#"{"message":"read ok"}"#);

    let context = get_context(1024 * 1024, "", "test_notfound", "");
    let read_res = test_service.read_(context.clone());
    assert_eq!(read_res.is_err(), true);
    let write_res = test_service.write_(context);
    assert_eq!(write_res.is_err(), true);

    test_service.hook_before_(&mock_executor_params()).unwrap();
    assert_eq!(test_service.hook_before, true);

    test_service.hook_after_(&mock_executor_params()).unwrap();
    assert_eq!(test_service.hook_after, true);
}

#[test]
fn test_service_none_response() {
    struct Tests<SDK: ServiceSDK> {
        _sdk:         SDK,
        genesis_data: String,
        hook_before:  bool,
        hook_after:   bool,
    }

    #[service]
    impl<SDK: ServiceSDK> Tests<SDK> {
        #[genesis]
        fn init_genesis(&mut self) -> ProtocolResult<()> {
            self.genesis_data = "genesis".to_owned();

            Ok(())
        }

        #[hook_before]
        fn custom_hook_before(&mut self, _params: &ExecutorParams) -> ProtocolResult<()> {
            self.hook_before = true;
            Ok(())
        }

        #[hook_after]
        fn custom_hook_after(&mut self, _params: &ExecutorParams) -> ProtocolResult<()> {
            self.hook_after = true;
            Ok(())
        }

        #[read]
        fn test_read(&self, _ctx: ServiceContext) -> ProtocolResult<()> {
            Ok(())
        }

        #[write]
        fn test_write(&mut self, _ctx: ServiceContext) -> ProtocolResult<()> {
            Ok(())
        }
    }

    let sdk = MockServiceSDK {};
    let mut test_service = Tests {
        _sdk:         sdk,
        genesis_data: "".to_owned(),
        hook_after:   false,
        hook_before:  false,
    };

    test_service.genesis_("".to_owned()).unwrap();
    assert_eq!(test_service.genesis_data, "genesis");

    let context = get_context(1024 * 1024, "", "test_write", "");
    let write_res = test_service.write_(context).unwrap();
    assert_eq!(write_res, "");

    let context = get_context(1024 * 1024, "", "test_read", "");
    let read_res = test_service.read_(context).unwrap();
    assert_eq!(read_res, "");

    let context = get_context(1024 * 1024, "", "test_notfound", "");
    let read_res = test_service.read_(context.clone());
    assert_eq!(read_res.is_err(), true);
    let write_res = test_service.write_(context);
    assert_eq!(write_res.is_err(), true);

    test_service.hook_before_(&mock_executor_params()).unwrap();
    assert_eq!(test_service.hook_before, true);

    test_service.hook_after_(&mock_executor_params()).unwrap();
    assert_eq!(test_service.hook_after, true);
}

fn get_context(cycles_limit: u64, service: &str, method: &str, payload: &str) -> ServiceContext {
    let params = ServiceContextParams {
        tx_hash: None,
        nonce: None,
        cycles_limit,
        cycles_price: 1,
        cycles_used: Rc::new(RefCell::new(0)),
        caller: Address::from_hash(Hash::from_empty()).unwrap(),
        epoch_id: 1,
        timestamp: 0,
        service_name: service.to_owned(),
        service_method: method.to_owned(),
        service_payload: payload.to_owned(),
        events: Rc::new(RefCell::new(vec![])),
    };

    ServiceContext::new(params)
}

fn mock_executor_params() -> ExecutorParams {
    ExecutorParams {
        state_root:   Hash::default(),
        epoch_id:     9,
        timestamp:    99,
        cycles_limit: 99999,
    }
}

struct MockServiceSDK;

impl ServiceSDK for MockServiceSDK {
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

    // Call other read-only methods of `service` and return the results
    // synchronously NOTE: You can use recursive calls, but the maximum call
    // stack is 1024
    fn read(
        &self,
        _ctx: &ServiceContext,
        _service: &str,
        _method: &str,
        _payload: &str,
    ) -> ProtocolResult<String> {
        unimplemented!()
    }

    // Call other writable methods of `service` and return the results synchronously
    // NOTE: You can use recursive calls, but the maximum call stack is 1024
    fn write(
        &mut self,
        _ctx: &ServiceContext,
        _service: &str,
        _method: &str,
        _payload: &str,
    ) -> ProtocolResult<String> {
        unimplemented!()
    }
}
