use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::{Send, Sync};

pub const P2P_SESSION_ID: &str = "p2p_session_id";

pub trait Cloneable: CloneableImpl + Debug + Send + Sync {}

pub trait CloneableImpl {
    fn box_clone(&self) -> Box<Cloneable>;
    fn as_any(&self) -> &Any;
}

impl<T> CloneableImpl for T
where
    T: 'static + Cloneable + Clone + Debug,
{
    fn box_clone(&self) -> Box<dyn Cloneable> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &Any {
        self
    }
}

impl Clone for Box<Cloneable> {
    fn clone(&self) -> Box<Cloneable> {
        self.box_clone()
    }
}

impl Cloneable for String {}
impl Cloneable for usize {}

pub trait CommonValue {
    fn p2p_session_id(&self) -> Option<usize>;
}

impl CommonValue for Context {
    fn p2p_session_id(&self) -> Option<usize> {
        self.get::<usize>(P2P_SESSION_ID).map(Clone::clone)
    }
}

/// Blockchain Context. eg. block, system contract.
#[derive(Clone, Debug, Default)]
pub struct Context {
    inner: HashMap<String, Box<dyn Cloneable>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            inner: HashMap::new(),
        }
    }

    pub fn with_value<V: 'static + Cloneable + Debug>(&self, key: &str, val: V) -> Self {
        let mut ctx = self.clone();

        ctx.inner.insert(key.to_owned(), val.box_clone());
        ctx
    }

    pub fn get<V: 'static>(&self, key: &str) -> Option<&V> {
        let opt_val = self.inner.get(key);

        opt_val.and_then(|any| any.as_any().downcast_ref::<V>())
    }
}

#[cfg(test)]
mod tests {
    use super::{CommonValue, Context, P2P_SESSION_ID};

    #[test]
    fn test_context() {
        let ctx = Context::new();

        let net_ctx = ctx.with_value::<usize>("session_id", 1);
        assert_eq!(net_ctx.get::<usize>("session_id"), Some(&1));

        let halo_ctx = net_ctx.with_value("spartan", "jonh117".to_owned());
        assert_eq!(halo_ctx.get("spartan").map(String::as_str), Some("jonh117"));
    }

    #[test]
    fn test_context_wrong_type() {
        let ctx = Context::new();

        let kingdom = ctx.with_value::<usize>("knights", 13);
        let micky_guess = kingdom.get::<u64>("knights");

        assert_eq!(micky_guess, None);
    }

    #[test]
    fn test_context_common_value() {
        let ctx = Context::new();
        assert_eq!(ctx.p2p_session_id(), None);

        let ctx = ctx.with_value::<usize>(P2P_SESSION_ID, 1);
        assert_eq!(ctx.p2p_session_id(), Some(1));
    }
}
