#[macro_use]
extern crate core_binding_macro;

use bytes::Bytes;
use json::JsonValue;

use protocol::traits::RequestContext;
use protocol::types::{Address, Hash};
use protocol::ProtocolResult;

#[test]
fn test_read() {
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

    let context = MockRequestContext {};

    let mut t = Tests {};
    assert_eq!(t.test_read_fn(context.clone()).unwrap(), JsonValue::Null);
    assert_eq!(t.test_write_fn(context.clone()).unwrap(), JsonValue::Null);
}

#[derive(Clone)]
struct MockRequestContext;

impl RequestContext for MockRequestContext {
    fn sub_cycles(&self, _cycels: u64) -> ProtocolResult<()> {
        Ok(())
    }

    fn get_cycles_price(&self) -> ProtocolResult<u64> {
        Ok(0)
    }

    fn get_cycles_limit(&self) -> ProtocolResult<u64> {
        Ok(0)
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
