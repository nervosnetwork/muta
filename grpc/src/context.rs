use grpc::{Metadata, MetadataKey, RequestOptions};

use crate::service::Context;

// Trait for Exchange the value of Context and RPC Context, in which,
// Context: a context maintain by the framework;
// RPC Context: a context for a specific GRPC implementation (grpc-rust, gprc-rs, tower-grpc, e.g.).
pub trait ContextExchange<T> {
    fn into_rpc_context(self) -> T;
    fn from_rpc_context(rpc_ctx: T) -> Context;
}

impl ContextExchange<RequestOptions> for Context {
    fn into_rpc_context(self) -> RequestOptions {
        let mut metadata = Metadata::new();

        for (key, value) in self.into_iter() {
            metadata.add(MetadataKey::from(key), value);
        }

        RequestOptions { metadata }
    }

    fn from_rpc_context(rpc_ctx: RequestOptions) -> Context {
        let mut context = Context::new();

        for e in rpc_ctx.metadata.entries {
            context.insert(e.key.as_str().to_owned(), e.value);
        }
        context
    }
}
