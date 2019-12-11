extern crate proc_macro;

mod read_write;

use proc_macro::TokenStream;

use crate::read_write::verify_read_or_write;

// `#[read]` marks a service method as readable.
//
// Methods marked with this macro will have:
//  Methods with this macro allow access (readable) from outside (RPC or other
// services).
//
// - Verification
//  1. Is it a struct method marked with #[service]?
//  2. Is visibility private?
//  3. Does function generics constrain `fn f<Context: RequestContext>`?
//  4. Parameter signature contains `&self and ctx:Context`?
//  5. Is the return value `ProtocolResult <JsonValue>`?
//
// example:
//
// struct Service;
// #[service]
// impl Service {
//     #[read]
//     fn test_read_fn<Context: RequestContext>(
//         &self,
//         _ctx: Context,
//     ) -> ProtocolResult<JsonValue> {
//         Ok(JsonValue::Null)
//     }
// }
#[proc_macro_attribute]
pub fn read(_: TokenStream, item: TokenStream) -> TokenStream {
    verify_read_or_write(item, false)
}

// `#[write]` marks a service method as writeable.
//
// Methods marked with this macro will have:
// - Accessibility
//  Methods with this macro allow access (writeable) from outside (RPC or other
// services).
//
// - Verification
//  1. Is it a struct method marked with #[service]?
//  2. Is visibility private?
//  3. Does function generics constrain `fn f<Context: RequestContext>`?
//  4. Parameter signature contains `&mut self and ctx:Context`?
//  5. Is the return value `ProtocolResult <JsonValue>`?
//
// example:
//
// struct Service;
// #[service]
// impl Service {
//     #[write]
//     fn test_write_fn<Context: RequestContext>(
//         &mut self,
//         _ctx: Context,
//     ) -> ProtocolResult<JsonValue> {
//         Ok(JsonValue::Null)
//     }
// }
#[proc_macro_attribute]
pub fn write(_: TokenStream, item: TokenStream) -> TokenStream {
    verify_read_or_write(item, true)
}
