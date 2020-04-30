#![allow(clippy::unit_cmp)]
#[macro_use]
extern crate binding_macro;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    ExecutorParams, SchemaGenerator, Service, ServiceResponse, ServiceSDK, StoreArray, StoreBool,
    StoreMap, StoreString, StoreUint64,
};
use protocol::types::{
    Address, Block, Hash, Receipt, ServiceContext, ServiceContextParams, SignedTransaction,
};

// use binding_macro::SchemaObject;

#[test]
fn test_read_and_write() {
    struct Tests;

    impl Tests {
        #[read]
        fn test_read_fn(&self, _ctx: ServiceContext) -> ServiceResponse<String> {
            ServiceResponse::<String>::from_succeed("read".to_owned())
        }

        #[write]
        fn test_write_fn(&mut self, _ctx: ServiceContext) -> ServiceResponse<String> {
            ServiceResponse::<String>::from_succeed("write".to_owned())
        }
    }

    let context = get_context(1000, "", "", "");

    let mut t = Tests {};
    assert_eq!(
        t.test_read_fn(context.clone()).succeed_data,
        "read".to_owned()
    );
    assert_eq!(t.test_write_fn(context).succeed_data, "write".to_owned());
}

#[test]
fn test_hooks() {
    struct Tests {
        pub height: u64,
    };

    impl Tests {
        #[hook_after]
        fn hook_after(&mut self, params: &ExecutorParams) -> ServiceResponse<()> {
            self.height = params.height;
            ServiceResponse::<()>::from_succeed(())
        }

        #[hook_before]
        fn hook_before(&mut self, params: &ExecutorParams) -> ServiceResponse<()> {
            self.height = params.height;
            ServiceResponse::<()>::from_succeed(())
        }
    }

    let mut t = Tests { height: 0 };
    t.hook_after(&mock_executor_params());
    assert_eq!(t.height, 9);
    t.hook_before(&mock_executor_params());
    assert_eq!(t.height, 9);
}

#[test]
fn test_read_and_write_with_noneparams() {
    struct Tests;

    impl Tests {
        #[read]
        fn test_read_fn(&self, _ctx: ServiceContext) -> ServiceResponse<()> {
            ServiceResponse::<()>::from_succeed(())
        }

        #[write]
        fn test_write_fn(&mut self, _ctx: ServiceContext) -> ServiceResponse<()> {
            ServiceResponse::<()>::from_succeed(())
        }
    }

    let context = get_context(1000, "", "", "");

    let mut t = Tests {};
    assert_eq!(t.test_read_fn(context.clone()).succeed_data, ());
    assert_eq!(t.test_write_fn(context).succeed_data, ());
}

#[test]
fn test_cycles() {
    struct Tests;

    impl Tests {
        #[cycles(100)]
        fn test_cycles(&self, ctx: ServiceContext) -> ServiceResponse<()> {
            ServiceResponse::<()>::from_succeed(())
        }

        #[cycles(500)]
        fn test_cycles2(&self, ctx: ServiceContext) -> ServiceResponse<()> {
            ServiceResponse::<()>::from_succeed(())
        }
    }

    #[cycles(200)]
    fn test_sub_cycles_fn1(ctx: ServiceContext) -> ServiceResponse<()> {
        ServiceResponse::<()>::from_succeed(())
    }

    #[cycles(200)]
    fn test_sub_cycles_fn2(_foo: u64, ctx: ServiceContext) -> ServiceResponse<()> {
        ServiceResponse::<()>::from_succeed(())
    }

    let t = Tests {};
    let context = get_context(1000, "", "", "");
    t.test_cycles(context.clone());
    assert_eq!(context.get_cycles_used(), 100);

    t.test_cycles2(context.clone());
    assert_eq!(context.get_cycles_used(), 600);

    test_sub_cycles_fn1(context.clone());
    assert_eq!(context.get_cycles_used(), 800);

    test_sub_cycles_fn2(1, context.clone());
    assert_eq!(context.get_cycles_used(), 1000);
}

#[test]
fn test_service() {
    #[derive(Serialize, Deserialize, Debug, SchemaObject)]
    struct TestServicePayload {
        name: String,
        age:  u64,
        sex:  bool,
    }
    #[derive(Serialize, Deserialize, Debug, Default, SchemaObject)]
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
        fn init_genesis(&mut self) {
            self.genesis_data = "genesis".to_owned();
        }

        #[hook_before]
        fn custom_hook_before(&mut self, _params: &ExecutorParams) {
            self.hook_before = true;
        }

        #[hook_after]
        fn custom_hook_after(&mut self, _params: &ExecutorParams) {
            self.hook_after = true;
        }

        #[read]
        fn test_read(
            &self,
            _ctx: ServiceContext,
            _payload: TestServicePayload,
        ) -> ServiceResponse<TestServiceResponse> {
            let res = TestServiceResponse {
                message: "read ok".to_owned(),
            };

            ServiceResponse::<TestServiceResponse>::from_succeed(res)
        }

        #[write]
        fn test_write(
            &mut self,
            _ctx: ServiceContext,
            _payload: TestServicePayload,
        ) -> ServiceResponse<TestServiceResponse> {
            let res = TestServiceResponse {
                message: "write ok".to_owned(),
            };

            ServiceResponse::<TestServiceResponse>::from_succeed(res)
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

    test_service.genesis_("".to_owned());
    assert_eq!(test_service.genesis_data, "genesis");

    let context = get_context(1024 * 1024, "", "test_write", &payload_str);
    let write_res = test_service.write_(context).succeed_data;
    assert_eq!(write_res, r#"{"message":"write ok"}"#);

    let context = get_context(1024 * 1024, "", "test_read", &payload_str);
    let read_res = test_service.read_(context).succeed_data;
    assert_eq!(read_res, r#"{"message":"read ok"}"#);

    let context = get_context(1024 * 1024, "", "test_notfound", &payload_str);
    let read_res = panic::catch_unwind(AssertUnwindSafe(|| test_service.read_(context.clone())));
    assert_eq!(read_res.unwrap().is_error(), true);
    let write_res = panic::catch_unwind(AssertUnwindSafe(|| test_service.write_(context)));
    assert_eq!(write_res.unwrap().is_error(), true);

    test_service.hook_before_(&mock_executor_params());
    assert_eq!(test_service.hook_before, true);

    test_service.hook_after_(&mock_executor_params());
    assert_eq!(test_service.hook_after, true);
}

#[test]
fn test_service_none_payload() {
    #[derive(Serialize, Deserialize, Debug, Default, SchemaObject)]
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
        fn init_genesis(&mut self) {
            self.genesis_data = "genesis".to_owned();
        }

        #[hook_before]
        fn custom_hook_before(&mut self, _params: &ExecutorParams) {
            self.hook_before = true;
        }

        #[hook_after]
        fn custom_hook_after(&mut self, _params: &ExecutorParams) {
            self.hook_after = true;
        }

        #[read]
        fn test_read(&self, _ctx: ServiceContext) -> ServiceResponse<TestServiceResponse> {
            let res = TestServiceResponse {
                message: "read ok".to_owned(),
            };

            ServiceResponse::<TestServiceResponse>::from_succeed(res)
        }

        #[write]
        fn test_write(&mut self, _ctx: ServiceContext) -> ServiceResponse<TestServiceResponse> {
            let res = TestServiceResponse {
                message: "write ok".to_owned(),
            };

            ServiceResponse::<TestServiceResponse>::from_succeed(res)
        }
    }

    let sdk = MockServiceSDK {};
    let mut test_service = Tests {
        _sdk:         sdk,
        genesis_data: "".to_owned(),
        hook_after:   false,
        hook_before:  false,
    };

    test_service.genesis_("".to_owned());
    assert_eq!(test_service.genesis_data, "genesis");

    let context = get_context(1024 * 1024, "", "test_write", "");
    let write_res = test_service.write_(context).succeed_data;
    assert_eq!(write_res, r#"{"message":"write ok"}"#);

    let context = get_context(1024 * 1024, "", "test_read", "");
    let read_res = test_service.read_(context).succeed_data;
    assert_eq!(read_res, r#"{"message":"read ok"}"#);

    let context = get_context(1024 * 1024, "", "test_notfound", "");
    let read_res = panic::catch_unwind(AssertUnwindSafe(|| test_service.read_(context.clone())));
    assert_eq!(read_res.unwrap().is_error(), true);
    let write_res = panic::catch_unwind(AssertUnwindSafe(|| test_service.write_(context)));
    assert_eq!(write_res.unwrap().is_error(), true);

    test_service.hook_before_(&mock_executor_params());
    assert_eq!(test_service.hook_before, true);

    test_service.hook_after_(&mock_executor_params());
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
        fn init_genesis(&mut self) {
            self.genesis_data = "genesis".to_owned();
        }

        #[hook_before]
        fn custom_hook_before(&mut self, _params: &ExecutorParams) {
            self.hook_before = true;
        }

        #[hook_after]
        fn custom_hook_after(&mut self, _params: &ExecutorParams) {
            self.hook_after = true;
        }

        #[read]
        fn test_read(&self, _ctx: ServiceContext) -> ServiceResponse<()> {
            ServiceResponse::<()>::from_succeed(())
        }

        #[write]
        fn test_write(&mut self, _ctx: ServiceContext) -> ServiceResponse<()> {
            ServiceResponse::<()>::from_succeed(())
        }
    }

    let sdk = MockServiceSDK {};
    let mut test_service = Tests {
        _sdk:         sdk,
        genesis_data: "".to_owned(),
        hook_after:   false,
        hook_before:  false,
    };

    test_service.genesis_("".to_owned());
    assert_eq!(test_service.genesis_data, "genesis");

    let context = get_context(1024 * 1024, "", "test_write", "");
    let write_res = test_service.write_(context).succeed_data;
    assert_eq!(write_res, "");

    let context = get_context(1024 * 1024, "", "test_read", "");
    let read_res = test_service.read_(context).succeed_data;
    assert_eq!(read_res, "");

    let context = get_context(1024 * 1024, "", "test_notfound", "");
    let read_res = panic::catch_unwind(AssertUnwindSafe(|| test_service.read_(context.clone())));
    assert_eq!(read_res.unwrap().is_error(), true);
    let write_res = panic::catch_unwind(AssertUnwindSafe(|| test_service.write_(context)));
    assert_eq!(write_res.unwrap().is_error(), true);

    test_service.hook_before_(&mock_executor_params());
    assert_eq!(test_service.hook_before, true);

    test_service.hook_after_(&mock_executor_params());
    assert_eq!(test_service.hook_after, true);
}

#[test]
fn test_schema() {
    #[derive(SchemaObject, Default, Serialize, Deserialize)]
    #[description("This is TestA")]
    struct TestA {
        #[description("This is String a")]
        a: String,
        #[description("This is TestB b")]
        b: TestB,
        #[description("This is bool c")]
        c: bool,
        #[description("This is u64 d")]
        d: u64,
    }
    #[derive(SchemaObject, Default, Serialize, Deserialize)]
    #[description("This is TestB")]
    struct TestB {
        #[description("This is Vec<u8> e")]
        e: Vec<u8>,
    }
    #[derive(SchemaObject, Default, Serialize, Deserialize)]
    #[description("This is TestEvent")]
    struct TestEvent {
        #[description("This is TestA f")]
        f: TestA,
    }

    #[derive(SchemaEvent)]
    enum Event {
        TestEvent,
    }

    struct TestService<SDK: ServiceSDK> {
        _sdk:         SDK,
        genesis_data: String,
    }

    #[service(Event)]
    impl<SDK: ServiceSDK> TestService<SDK> {
        #[genesis]
        fn init_genesis(&mut self) {
            self.genesis_data = "genesis".to_owned();
        }

        #[read]
        fn test_read(&self, _ctx: ServiceContext) -> ServiceResponse<TestA> {
            ServiceResponse::<TestA>::from_error(1, "error".to_owned())
        }

        #[write]
        fn test_write(&mut self, _ctx: ServiceContext, _payload: TestB) -> ServiceResponse<()> {
            let _place_holder = Event::TestEvent;
            ServiceResponse::<()>::from_succeed(())
        }
    }

    let sdk = MockServiceSDK {};
    let test_service = TestService {
        _sdk:         sdk,
        genesis_data: "".to_owned(),
    };

    let method_schema_expected = "type Mutation {\n  test_write(\n    payload: TestB!\n  ): Null\n}\n\ntype Query {\n  test_read: TestA!\n}\n\n# This is TestA\ntype TestA {\n  # This is String a\n  a: String!\n  # This is TestB b\n  b: TestB!\n  # This is bool c\n  c: Boolean!\n  # This is u64 d\n  d: U64!\n}\n\n# This is TestB\ntype TestB {\n  # This is Vec<u8> e\n  e: [U8!]!\n}\n\nscalar U64\n\nscalar U8\n\nscalar Null\n\n";
    let event_schema_expected = "# This is TestA\ntype TestA {\n  # This is String a\n  a: String!\n  # This is TestB b\n  b: TestB!\n  # This is bool c\n  c: Boolean!\n  # This is u64 d\n  d: U64!\n}\n\n# This is TestB\ntype TestB {\n  # This is Vec<u8> e\n  e: [U8!]!\n}\n\n# This is TestEvent\ntype TestEvent {\n  # This is TestA f\n  f: TestA!\n}\n\nscalar U64\n\nscalar U8\n\nunion Event = TestEvent\n\n";
    assert_eq!(test_service.schema_().0, method_schema_expected);
    assert_eq!(test_service.schema_().1, event_schema_expected);
}

fn get_context(cycles_limit: u64, service: &str, method: &str, payload: &str) -> ServiceContext {
    let params = ServiceContextParams {
        tx_hash: None,
        nonce: None,
        cycles_limit,
        cycles_price: 1,
        cycles_used: Rc::new(RefCell::new(0)),
        caller: Address::from_hash(Hash::from_empty()).unwrap(),
        height: 1,
        timestamp: 0,
        service_name: service.to_owned(),
        service_method: method.to_owned(),
        service_payload: payload.to_owned(),
        extra: None,
        events: Rc::new(RefCell::new(vec![])),
    };

    ServiceContext::new(params)
}

fn mock_executor_params() -> ExecutorParams {
    ExecutorParams {
        state_root:   Hash::default(),
        height:       9,
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
    ) -> Box<dyn StoreMap<Key, Val>> {
        unimplemented!()
    }

    // Alloc or recover a `Array` by` var_name`
    fn alloc_or_recover_array<Elm: 'static + FixedCodec>(
        &mut self,
        _var_name: &str,
    ) -> Box<dyn StoreArray<Elm>> {
        unimplemented!()
    }

    // Alloc or recover a `U64` by` var_name`
    fn alloc_or_recover_uint64(&mut self, _var_name: &str) -> Box<dyn StoreUint64> {
        unimplemented!()
    }

    // Alloc or recover a `String` by` var_name`
    fn alloc_or_recover_string(&mut self, _var_name: &str) -> Box<dyn StoreString> {
        unimplemented!()
    }

    // Alloc or recover a `Bool` by` var_name`
    fn alloc_or_recover_bool(&mut self, _var_name: &str) -> Box<dyn StoreBool> {
        unimplemented!()
    }

    // Get a value from the service state by key
    fn get_value<Key: FixedCodec, Ret: FixedCodec>(&self, _key: &Key) -> Option<Ret> {
        unimplemented!()
    }

    // Set a value to the service state by key
    fn set_value<Key: FixedCodec, Val: FixedCodec>(&mut self, _key: Key, _val: Val) {
        unimplemented!()
    }

    // Get a value from the specified address by key
    fn get_account_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        _address: &Address,
        _key: &Key,
    ) -> Option<Ret> {
        unimplemented!()
    }

    // Insert a pair of key / value to the specified address
    fn set_account_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        _address: &Address,
        _key: Key,
        _val: Val,
    ) {
        unimplemented!()
    }

    // Get a signed transaction by `tx_hash`
    // if not found on the chain, return None
    fn get_transaction_by_hash(&self, _tx_hash: &Hash) -> Option<SignedTransaction> {
        unimplemented!()
    }

    // Get a block by `height`
    // if not found on the chain, return None
    // When the parameter `height` is None, get the latest (executing)` block`
    fn get_block_by_height(&self, _height: Option<u64>) -> Option<Block> {
        unimplemented!()
    }

    // Get a receipt by `tx_hash`
    // if not found on the chain, return None
    fn get_receipt_by_hash(&self, _tx_hash: &Hash) -> Option<Receipt> {
        unimplemented!()
    }

    // Call other read-only methods of `service` and return the results
    // synchronously NOTE: You can use recursive calls, but the maximum call
    // stack is 1024
    fn read(
        &self,
        _ctx: &ServiceContext,
        _extra: Option<Bytes>,
        _service: &str,
        _method: &str,
        _payload: &str,
    ) -> ServiceResponse<String> {
        unimplemented!()
    }

    // Call other writable methods of `service` and return the results synchronously
    // NOTE: You can use recursive calls, but the maximum call stack is 1024
    fn write(
        &mut self,
        _ctx: &ServiceContext,
        _extra: Option<Bytes>,
        _service: &str,
        _method: &str,
        _payload: &str,
    ) -> ServiceResponse<String> {
        unimplemented!()
    }
}
