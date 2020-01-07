extern crate proc_macro;

#[macro_use]
extern crate static_assertions;

mod common;
mod cycles;
mod read_write;
mod service;

use proc_macro::TokenStream;

use crate::cycles::gen_cycles_code;
use crate::read_write::verify_read_or_write;
use crate::service::gen_service_code;

#[rustfmt::skip]
/// `#[read]` marks a service method as readable.
///
/// Methods marked with this macro will have:
///  Methods with this macro allow access (readable) from outside (RPC or other services).
///
/// - Verification
///  1. Is it a struct method marked with #[service]?
///  2. Is visibility private?
///  3. Parameter signature contains `&self and ctx:ServiceContext`?
///  4. Is the return value `ProtocolResult <T: Deserialize + Serialize>` or `ProtocolResult <()>`?
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
///         _ctx: Context,
///     ) -> ProtocolResult<String> {
///         Ok("test read".to_owend())
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
///  3. Parameter signature contains `&self and ctx:ServiceContext`?
///  4. Is the return value `ProtocolResult <T: Deserialize + Serialize>` or `ProtocolResult <()>`?
///
/// # Example:
///
/// ```rust
/// struct Service;
/// #[service]
/// impl Service {
///     #[write]
///     fn test_write_fn<Context: RequestContext>(
///         &mut self,
///         _ctx: Context,
///     ) -> ProtocolResult<String> {
///         Ok("test write".to_owned())
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
///     fn test_cycles<Context: RequestContext>(&self, ctx: Context) -> ProtocolResult<()> {
///         Ok(())
///     }
/// }
///
/// // Generated code.
/// impl Tests {
///     fn test_cycles<Context: RequestContext>(&self, ctx: Context) -> ProtocolResult<()> {
///         ctx.sub_cycles(100)?;
///         Ok(())
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
    item
}

/// Marks a method so that it executes before the entire block executes.
// TODO(@yejiayu): Verify the function signature.
#[proc_macro_attribute]
pub fn hook_before(_: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[rustfmt::skip]
/// Marking a ImplItem for service, it will automatically trait
/// `protocol::traits::Service`.
///
/// # Example
///
/// use serde::{Deserialize, Serialize};
/// use protocol::traits::{RequestContext, ServiceSDK};
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
///     fn custom_hook_before(&mut self) -> ProtoResult<()> {
///         // Do something
///     }
///
///     #[hook_after]
///     fn custom_hook_after(&mut self) -> ProtoResult<()> {
///         // Do something
///     }
///
///     #[read]
///     fn get_kitty<Context: RequestContext>(
///         &self,
///         ctx: Context,
///         payload: GetKittyPayload,
///     ) -> ProtoResult<&str> {
///         // Do something
///     }
///
///     #[write]
///     fn create_kitty<Context: RequestContext>(
///         &mut self,
///         ctx: Context,
///         payload: CreateKittyPayload,
///     ) -> ProtoResult<&str> {
///         // Do something
///     }
/// }
///
/// // Generated code.
/// impl<SDK: ServiceSDK> Service<SDK> for KittyService<SDK> {
///     fn hook_before_(&mut self) -> ProtocolResult<()> {
///         self.custom_hook_before()
///     }
///
///     fn hook_after(&mut self) -> ProtocolResult<()> {
///         self.custom_hook_after()
///     }
///
///     fn write<Context: RequestContext>(&mut self, ctx: Context) -> ProtocolResult<&str> {
///         let method = ctx.get_service_method();
///
///         match ctx.get_service_method() {
///             "create_kitty" => {
///                 let payload: CreateKittyPayload = serde_json::from_str(ctx.get_payload())
///                     .map_err(|e| core_binding::ServiceError::JsonParse(e))?;
///                 let res = self.create_kitty(ctx, payload)?;
///                 serde_json::to_string(&res).map_err(|e| framework::ServiceError::JsonParse(e).into())
///             }
///             _ => Err(core_binding::ServiceError::NotFoundMethod(method.to_owned()).into()),
///         }
///     }
///
///     fn read<Context: RequestContext>(&self, ctx: Context) -> ProtocolResult<&str> {
///         let method = ctx.get_service_method();
///
///         match ctx.get_service_method() {
///             "get_kitty" => {
///                 let payload: GetKittyPayload = serde_json::from_str(ctx.get_payload())
///                     .map_err(|e| core_binding::ServiceError::JsonParse(e))?;
///                 let res = self.get_kitty(ctx, payload)?;
///                 serde_json::to_string(&res).map_err(|e| framework::ServiceError::JsonParse(e).into())
///             }
///             _ => Err(core_binding::ServiceError::NotFoundMethod(method.to_owned()).into()),
///         }
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn service(attr: TokenStream, item: TokenStream) -> TokenStream {
    gen_service_code(attr, item)
}
