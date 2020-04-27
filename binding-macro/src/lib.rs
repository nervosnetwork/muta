extern crate proc_macro;

mod common;
mod cycles;
mod hooks;
mod read_write;
mod schema;
mod service;

use proc_macro::TokenStream;

use crate::cycles::gen_cycles_code;
use crate::hooks::verify_hook;
use crate::read_write::verify_read_or_write;
use crate::schema::impl_event;
use crate::schema::impl_object;
use crate::service::gen_service_code;

#[rustfmt::skip]
/// `#[genesis]` marks a service method to generate genesis states when fire up the chain
///
/// Method input params should be `(&mut self)` or `(&mut self, payload: PayloadType)`
///
/// # Example:
///
/// ```rust
/// struct Service;
/// #[service]
/// impl Service {
///     #[genesis]
///     fn init_genesis(
///         &mut self,
///     ) {
///         do_work();
///     }
/// }
/// ```
///
/// Or
///
/// ```rust
/// struct Service;
/// #[service]
/// impl Service {
///     #[genesis]
///     fn init_genesis(
///         &mut self,
///         payload: PayloadType,
///     ) {
///         do_work(payload);
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn genesis(_: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn tx_hook_before(_: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn tx_hook_after(_: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[rustfmt::skip]
/// `#[read]` marks a service method as readable.
///
/// Methods marked with this macro will have:
///  Methods with this macro allow access (readable) from outside (RPC or other services).
///
/// - Verification
///  1. Is it a struct method marked with #[service]?
///  2. Is visibility private?
///  3. Parameter signature contains `&self and ctx: ServiceContext`?
///  4. Is the return value `ServiceResponse<T>`?
///
/// # Example:
///
/// ```rust
/// struct Service;
/// #[service]
/// impl Service {
///     #[read]
///     fn test_read_fn(
///         &self,
///         _ctx: ServiceContext,
///     ) -> ServiceResponse<String> {
///         ServiceResponse::<String>::from_succeed("ok".to_owned())
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn read(_: TokenStream, item: TokenStream) -> TokenStream {
    verify_read_or_write(item, false)
}

#[rustfmt::skip]
/// `#[write]` marks a service method as writable.
///
/// Methods marked with this macro will have:
/// - Accessibility
///  Methods with this macro allow access (writeable) from outside (RPC or other services).
///
/// - Verification
///  1. Is it a struct method marked with #[service]?
///  2. Is visibility private?
///  3. Parameter signature contains `&self and ctx: ServiceContext`?
///  4. Is the return value `ServiceResponse<T>`?
///
/// # Example:
///
/// ```rust
/// struct Service;
/// #[service]
/// impl Service {
///     #[write]
///     fn test_write_fn(
///         &mut self,
///         _ctx: ServiceContext,
///     ) -> ServiceResponse<String> {
///         ServiceResponse::<String>::from_succeed("ok".to_owned())
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn write(_: TokenStream, item: TokenStream) -> TokenStream {
    verify_read_or_write(item, true)
}

#[rustfmt::skip]
/// `# [cycles]` mark an `ImplFn` or `fn`, it will automatically generate code
/// to complete the cycle deduction,
///
/// ```rust
/// // Source Code
/// impl Tests {
///     #[cycles(100)]
///     fn test_cycles(&self, ctx: ServiceContext) -> ServiceResponse<()> {
///         ServiceResponse::<()>::from_succeed(())
///     }
/// }
///
/// // Generated code.
/// impl Tests {
///     fn test_cycles(&self, ctx: ServiceContext) -> ServiceResponse<()> {
///         ctx.sub_cycles(100);
///         ServiceResponse::<()>::from_succeed(())
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn cycles(attr: TokenStream, item: TokenStream) -> TokenStream {
    gen_cycles_code(attr, item)
}

/// Marks a method so that it executes after the entire block executes.
// TODO(@yejiayu): Verify the function signature.
#[proc_macro_attribute]
pub fn hook_after(_: TokenStream, item: TokenStream) -> TokenStream {
    verify_hook(item)
}

/// Marks a method so that it executes before the entire block executes.
// TODO(@yejiayu): Verify the function signature.
#[proc_macro_attribute]
pub fn hook_before(_: TokenStream, item: TokenStream) -> TokenStream {
    verify_hook(item)
}

#[rustfmt::skip]
/// Marking a ImplItem for service, it will automatically trait
/// `protocol::traits::Service`.
///
/// # Example
///
/// use serde::{Deserialize, Serialize};
/// use protocol::traits::ServiceSDK;
/// use protocol::types::ServiceContext;
/// use protocol::ProtocolResult;
///
/// ```rust
/// // Source code
///
/// // serde::Deserialize and serde::Serialize are required.
/// #[derive(Serialize, Deserialize)]
/// struct CreateKittyPayload {
///     // fields
/// }
///
/// // serde::Deserialize and serde::Serialize are required.
/// #[derive(Serialize, Deserialize)]
/// struct GetKittyPayload<SDK: ServiceSDK> {
///     // fields
/// }
///
/// #[service]
/// impl<SDK: ServiceSDK> KittyService<SDK> {
///     #[hook_before]
///     fn custom_hook_before(&mut self) {
///         // Do something
///     }
///
///     #[hook_after]
///     fn custom_hook_after(&mut self) {
///         // Do something
///     }
///
///     #[read]
///     fn get_kitty(
///         &self,
///         ctx: ServiceContext,
///         payload: GetKittyPayload,
///     ) -> ServiceResponse<String> {
///         // Do something
///     }
///
///     #[write]
///     fn create_kitty(
///         &mut self,
///         ctx: ServiceContext,
///         payload: CreateKittyPayload,
///     ) -> ServiceResponse<String> {
///         // Do something
///     }
/// }
///
/// // Generated code.
/// impl<SDK: ServiceSDK> Service<SDK> for KittyService<SDK> {
///     fn hook_before_(&mut self) {
///         self.custom_hook_before()
///     }
///
///     fn hook_after(&mut self) {
///         self.custom_hook_after()
///     }
///
///     fn write(&mut self, ctx: ServiceContext) -> ServiceResponse<String> {
///         let method = ctx.get_service_method();
///
///         match ctx.get_service_method() {
///             "create_kitty" => {
///                 let payload_res: Result<CreateKittyPayload, _> = serde_json::from_str(ctx.get_payload());
///                 if payload_res.is_error() {
///                      return ServiceResponse::<String>::from_error(1, "service macro decode payload failed".to_owned());
///                 };
///                 let payload = payload_res.unwrap();
///                 let res = self.#list_read_ident(ctx, payload);
///                 if !res.is_error() {
///                     let mut data_json = serde_json::to_string(&res.succeed_data).unwrap_or_else(|e| panic!("service macro encode payload failed: {:?}", e));
///                     if data_json == "null" {
///                         data_json = "".to_owned();
///                     }
///                     ServiceResponse::<String>::from_succeed(data_json)
///                 } else {
///                     ServiceResponse::<String>::from_error(res.code, res.error_message.clone())
///             }
///             _ => panic!("service macro not found method:{:?} of service:{:?}", method, service),
///         }
///     }
///
///     fn read(&self, ctx: ServiceContext) -> ProtocolResult<&str> {
///         let method = ctx.get_service_method();
///
///         match ctx.get_service_method() {
///             "get_kitty" => {
///                 let payload_res: Result<GetKittyPayload, _> = serde_json::from_str(ctx.get_payload());
///                 if payload_res.is_error() {
///                      return ServiceResponse::<String>::from_error(1, "service macro decode payload failed".to_owned());
///                 };
///                 let payload = payload_res.unwrap();
///                 let res = self.#list_read_ident(ctx, payload);
///                 if !res.is_error() {
///                     let mut data_json = serde_json::to_string(&res.succeed_data).unwrap_or_else(|e| panic!("service macro encode payload failed: {:?}", e));
///                     if data_json == "null" {
///                         data_json = "".to_owned();
///                     }
///                     ServiceResponse::<String>::from_succeed(data_json)
///                 } else {
///                     ServiceResponse::<String>::from_error(res.code, res.error_message.clone())
///             }
///             _ => panic!("service macro not found method:{:?} of service:{:?}", method, service),
///         }
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn service(attr: TokenStream, item: TokenStream) -> TokenStream {
    gen_service_code(attr, item)
}

#[rustfmt::skip]
/// `#[derive(SchemaObject)]` marks a `struct`, which is payload or response of service interface method, to generate GraphQL schema.
/// 
/// GraphQL schema can be used by toolchain to generate ts-sdk or others.
/// 
/// # Example
/// 
/// ```rust
/// #[derive(SchemaObject)]
/// #[description("Transfer method payload")]
/// pub struct TransferPayload {
///     #[description("Asset id to be transfered")]
///     pub asset_id: Hash,
///     #[description("Receiver of transfer action")]
///     pub to:       Address,
///     #[description("Amount of transfer action")]
///     pub value:    u64,
/// }
/// ```
/// 
/// This will generate GraphQL schema:
/// 
/// ```graphql
/// // Transfer method payload
/// type TransferPayload {
///   // Asset id to be transfered
///   asset_id: Hash!
///   // Receiver of transfer action
///   to: Address!
///   // Amount of transfer action
///   value: Uint64!
/// }
/// ```
#[proc_macro_derive(SchemaObject, attributes(description))]
pub fn schema_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_object(&ast)
}

#[proc_macro_derive(SchemaEvent)]
pub fn event_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_event(&ast)
}
