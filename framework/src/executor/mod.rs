mod error;
mod factory;
#[cfg(test)]
mod tests;

pub use factory::ServiceExecutorFactory;

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    panic::{self, AssertUnwindSafe},
    rc::Rc,
    sync::Arc,
};

use cita_trie::DB as TrieDB;

use common_apm::muta_apm;
use protocol::traits::{
    Context, Dispatcher, Executor, ExecutorParams, ExecutorResp, NoopDispatcher, Service,
    ServiceMapping, ServiceResponse, ServiceState, Storage,
};
use protocol::types::{
    Address, Hash, MerkleRoot, Receipt, ReceiptResponse, ServiceContext, ServiceContextParams,
    ServiceParam, SignedTransaction, TransactionRequest,
};
use protocol::{ProtocolError, ProtocolResult};

use crate::binding::sdk::{DefaultChainQuerier, DefaultServiceSDK};
use crate::binding::state::{GeneralServiceState, MPTTrie};
use crate::executor::error::ExecutorError;

const SERVICE_NOT_FOUND_CODE: u64 = 62077;

trait TxHooks {
    fn before(
        &mut self,
        _: Context,
        _: ServiceContext,
    ) -> ProtocolResult<Vec<ServiceResponse<String>>> {
        Ok(vec![ServiceResponse::from_succeed(
            "default_implement".to_owned(),
        )])
    }

    fn after(
        &mut self,
        _: Context,
        _: ServiceContext,
    ) -> ProtocolResult<Vec<ServiceResponse<String>>> {
        Ok(vec![ServiceResponse::from_succeed(
            "default_implement".to_owned(),
        )])
    }
}

impl TxHooks for () {}

enum HookType {
    Before,
    After,
}

#[derive(Clone, Copy)]
enum ExecType {
    Read,
    Write,
}

struct ServiceStateMap<DB: TrieDB>(HashMap<String, Rc<RefCell<GeneralServiceState<DB>>>>);

impl<DB: TrieDB> ServiceStateMap<DB> {
    fn new() -> ServiceStateMap<DB> {
        Self(HashMap::new())
    }
}

impl<DB: TrieDB> Deref for ServiceStateMap<DB> {
    type Target = HashMap<String, Rc<RefCell<GeneralServiceState<DB>>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<DB: TrieDB> DerefMut for ServiceStateMap<DB> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<DB: TrieDB> ServiceStateMap<DB> {
    fn stash(&self) -> ProtocolResult<()> {
        for state in self.0.values() {
            state.borrow_mut().stash()?;
        }

        Ok(())
    }

    fn revert_cache(&self) -> ProtocolResult<()> {
        for state in self.0.values() {
            state.borrow_mut().revert_cache()?;
        }

        Ok(())
    }
}

struct CommitHooks<DB: TrieDB> {
    inner:  Vec<Box<dyn Service>>,
    states: Rc<ServiceStateMap<DB>>,
}

impl<DB: TrieDB> CommitHooks<DB> {
    fn new(hooks: Vec<Box<dyn Service>>, states: Rc<ServiceStateMap<DB>>) -> CommitHooks<DB> {
        Self {
            inner: hooks,
            states,
        }
    }

    // bagua kan 101 :)
    fn kan<H: FnOnce() -> ServiceResponse<String>>(
        _context: ServiceContext,
        states: Rc<ServiceStateMap<DB>>,
        hook: H,
    ) -> ProtocolResult<ServiceResponse<String>> {
        match panic::catch_unwind(AssertUnwindSafe(hook)) {
            Ok(res) => {
                states.stash()?;

                Ok(res)
            }
            Err(e) => {
                states.revert_cache()?;
                // something really bad happens, chain maybe fork, must halt
                Err(ProtocolError::from(ExecutorError::TxHook(e)))
            }
        }
    }
}

impl<DB: TrieDB> TxHooks for CommitHooks<DB> {
    fn before(
        &mut self,
        _context: Context,
        service_context: ServiceContext,
    ) -> ProtocolResult<Vec<ServiceResponse<String>>> {
        let mut ret: Vec<ServiceResponse<String>> = Vec::new();
        for hook in self.inner.iter_mut() {
            let resp = Self::kan(service_context.clone(), Rc::clone(&self.states), || {
                hook.tx_hook_before_(service_context.clone())
            })?;
            ret.push(resp);
        }

        Ok(ret)
    }

    fn after(
        &mut self,
        _context: Context,
        service_context: ServiceContext,
    ) -> ProtocolResult<Vec<ServiceResponse<String>>> {
        let mut ret: Vec<ServiceResponse<String>> = Vec::new();

        for hook in self.inner.iter_mut() {
            let resp = Self::kan(service_context.clone(), Rc::clone(&self.states), || {
                hook.tx_hook_after_(service_context.clone())
            })?;
            ret.push(resp);
        }

        Ok(ret)
    }
}

pub struct ServiceExecutor<S: Storage, DB: TrieDB, Mapping: ServiceMapping> {
    service_mapping: Arc<Mapping>,
    querier:         Rc<DefaultChainQuerier<S>>,
    states:          Rc<ServiceStateMap<DB>>,
    root_state:      Rc<RefCell<GeneralServiceState<DB>>>,
}

impl<S: Storage, DB: TrieDB, Mapping: ServiceMapping> Clone for ServiceExecutor<S, DB, Mapping> {
    fn clone(&self) -> Self {
        Self {
            service_mapping: Arc::clone(&self.service_mapping),
            querier:         Rc::clone(&self.querier),
            states:          Rc::clone(&self.states),
            root_state:      Rc::clone(&self.root_state),
        }
    }
}

impl<S: 'static + Storage, DB: 'static + TrieDB, Mapping: 'static + ServiceMapping>
    ServiceExecutor<S, DB, Mapping>
{
    pub fn create_genesis(
        services: Vec<ServiceParam>,
        trie_db: Arc<DB>,
        storage: Arc<S>,
        mapping: Arc<Mapping>,
    ) -> ProtocolResult<MerkleRoot> {
        let querier = Rc::new(DefaultChainQuerier::new(Arc::clone(&storage)));

        let mut states = ServiceStateMap::new();
        for name in mapping.list_service_name().into_iter() {
            let trie = MPTTrie::new(Arc::clone(&trie_db));

            states.insert(name, Rc::new(RefCell::new(GeneralServiceState::new(trie))));
        }

        for params in services.into_iter() {
            let state = states
                .get(&params.name)
                .ok_or(ExecutorError::NotFoundService {
                    service: params.name.to_owned(),
                })?;
            let sdk =
                DefaultServiceSDK::new(Rc::clone(state), Rc::clone(&querier), NoopDispatcher {});

            let mut service = mapping.get_service(&params.name, sdk)?;
            panic::catch_unwind(AssertUnwindSafe(|| {
                service.genesis_(params.payload.clone())
            }))
            .map_err(|e| ProtocolError::from(ExecutorError::InitService(format!("{:?}", e))))?;

            state.borrow_mut().stash()?;
        }

        let trie = MPTTrie::new(Arc::clone(&trie_db));
        let mut root_state = GeneralServiceState::new(trie);
        for (name, state) in states.iter() {
            let root = state.borrow_mut().commit()?;
            root_state.insert(name.to_owned(), root)?;
        }
        root_state.stash()?;
        root_state.commit()
    }

    pub fn with_root(
        root: MerkleRoot,
        trie_db: Arc<DB>,
        storage: Arc<S>,
        service_mapping: Arc<Mapping>,
    ) -> ProtocolResult<Self> {
        let trie = MPTTrie::from(root, Arc::clone(&trie_db))?;
        let root_state = GeneralServiceState::new(trie);

        let mut states = ServiceStateMap::new();
        for name in service_mapping.list_service_name().into_iter() {
            let trie = match root_state.get(&name)? {
                Some(service_root) => MPTTrie::from(service_root, Arc::clone(&trie_db))?,
                None => MPTTrie::new(Arc::clone(&trie_db)),
            };

            let service_state = GeneralServiceState::new(trie);
            states.insert(name.to_owned(), Rc::new(RefCell::new(service_state)));
        }

        Ok(Self {
            service_mapping,
            querier: Rc::new(DefaultChainQuerier::new(storage)),
            states: Rc::new(states),
            root_state: Rc::new(RefCell::new(root_state)),
        })
    }

    #[muta_apm::derive::tracing_span(kind = "executor.commit")]
    fn commit(&mut self, ctx: Context) -> ProtocolResult<MerkleRoot> {
        for (name, state) in self.states.iter() {
            let root = state.borrow_mut().commit()?;
            self.root_state.borrow_mut().insert(name.to_owned(), root)?;
        }
        self.root_state.borrow_mut().stash()?;
        self.root_state.borrow_mut().commit()
    }

    fn stash(&mut self) -> ProtocolResult<()> {
        self.states.stash()
    }

    fn revert_cache(&mut self) -> ProtocolResult<()> {
        self.states.revert_cache()
    }

    #[muta_apm::derive::tracing_span(
        kind = "executor.before_hook",
        tags = "{'hook_type': 'hook_type'}"
    )]
    fn hook(
        &mut self,
        ctx: Context,
        hook_type: HookType,
        exec_params: &ExecutorParams,
    ) -> ProtocolResult<()> {
        for name in self.service_mapping.list_service_name().into_iter() {
            let sdk = self.get_sdk(&name)?;
            let mut service = self.service_mapping.get_service(name.as_str(), sdk)?;

            let hook_ret = match hook_type {
                HookType::Before => {
                    panic::catch_unwind(AssertUnwindSafe(|| service.hook_before_(exec_params)))
                }
                HookType::After => {
                    panic::catch_unwind(AssertUnwindSafe(|| service.hook_after_(exec_params)))
                }
            };

            if hook_ret.is_err() {
                self.revert_cache()?;
            } else {
                self.stash()?;
            }
        }
        Ok(())
    }

    fn get_sdk(
        &self,
        service: &str,
    ) -> ProtocolResult<DefaultServiceSDK<GeneralServiceState<DB>, DefaultChainQuerier<S>, Self>>
    {
        let state = self
            .states
            .get(service)
            .ok_or(ExecutorError::NotFoundService {
                service: service.to_owned(),
            })?;

        Ok(DefaultServiceSDK::new(
            Rc::clone(&state),
            Rc::clone(&self.querier),
            (*self).clone(),
        ))
    }

    fn get_context(
        &self,
        tx_hash: Option<Hash>,
        nonce: Option<Hash>,
        caller: &Address,
        cycles_price: u64,
        cycles_limit: u64,
        params: &ExecutorParams,
        request: &TransactionRequest,
    ) -> ProtocolResult<ServiceContext> {
        let ctx_params = ServiceContextParams {
            tx_hash,
            nonce,
            cycles_limit,
            cycles_price,
            cycles_used: Rc::new(RefCell::new(0)),
            caller: caller.clone(),
            height: params.height,
            timestamp: params.timestamp,
            service_name: request.service_name.to_owned(),
            service_method: request.method.to_owned(),
            service_payload: request.payload.to_owned(),
            extra: None,
            events: Rc::new(RefCell::new(vec![])),
        };

        Ok(ServiceContext::new(ctx_params))
    }

    fn get_tx_hooks(&self, exec_type: ExecType) -> Box<dyn TxHooks> {
        match exec_type {
            ExecType::Read => Box::new(()),
            ExecType::Write => {
                let mut tx_hooks = vec![];

                for name in self.service_mapping.list_service_name().into_iter() {
                    let sdk = self
                        .get_sdk(&name)
                        .unwrap_or_else(|e| panic!("get target service sdk failed: {}", e));

                    let tx_hook_service = self
                        .service_mapping
                        .get_service(name.as_str(), sdk)
                        .unwrap_or_else(|e| panic!("get target service sdk failed: {}", e));

                    tx_hooks.push(tx_hook_service);
                }

                let hooks = CommitHooks::new(tx_hooks, Rc::clone(&self.states));
                Box::new(hooks)
            }
        }
    }

    fn catch_call(
        &mut self,
        context: Context,
        service_context: ServiceContext,
        exec_type: ExecType,
    ) -> ProtocolResult<ServiceResponse<String>> {
        let mut tx_hooks = self.get_tx_hooks(exec_type);

        let resp = tx_hooks.before(context.clone(), service_context.clone())?;
        self.states.stash()?;

        if resp.iter().filter(|r| r.is_error()).count() > 0 {
            tx_hooks.after(context, service_context)?;
            self.states.stash()?;
            return Ok(ServiceResponse::from_error(65535, "skip_tx_run".to_owned()));
        };

        let ret = match panic::catch_unwind(AssertUnwindSafe(|| {
            self.call(service_context.clone(), exec_type)
        })) {
            Ok(r) => {
                self.stash()?;
                Ok(r)
            }
            Err(e) => {
                self.revert_cache()?;
                log::error!("inner chain error occurred when calling service: {:?}", e);
                Err(ProtocolError::from(ExecutorError::CallService(format!(
                    "{:?}",
                    e
                ))))
            }
        }?;

        tx_hooks.after(context, service_context)?;
        self.states.stash()?;

        Ok(ret)
    }

    fn call(&self, context: ServiceContext, exec_type: ExecType) -> ServiceResponse<String> {
        let service_name = context.get_service_name();

        let sdk = match self.get_sdk(&service_name) {
            Ok(sdk) => sdk,
            Err(e) => return ServiceResponse::from_error(SERVICE_NOT_FOUND_CODE, e.to_string()),
        };
        let mut service = match self.service_mapping.get_service(&service_name, sdk) {
            Ok(s) => s,
            Err(e) => return ServiceResponse::from_error(SERVICE_NOT_FOUND_CODE, e.to_string()),
        };

        match exec_type {
            ExecType::Read => service.read_(context),
            ExecType::Write => service.write_(context),
        }
    }
}

impl<S: 'static + Storage, DB: 'static + TrieDB, Mapping: 'static + ServiceMapping> Executor
    for ServiceExecutor<S, DB, Mapping>
{
    #[muta_apm::derive::tracing_span(kind = "executor.exec", logs = "{'tx_len': 'txs.len()'}")]
    fn exec(
        &mut self,
        ctx: Context,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp> {
        self.hook(ctx.clone(), HookType::Before, params)?;

        let mut receipts = txs
            .iter()
            .map(|stx| {
                let service_context = self.get_context(
                    Some(stx.tx_hash.clone()),
                    Some(stx.raw.nonce.clone()),
                    &stx.raw.sender,
                    stx.raw.cycles_price,
                    stx.raw.cycles_limit,
                    params,
                    &stx.raw.request,
                )?;

                let exec_resp =
                    self.catch_call(ctx.clone(), service_context.clone(), ExecType::Write)?;
                let events = if exec_resp.is_error() {
                    Vec::new()
                } else {
                    service_context.get_events()
                };

                Ok(Receipt {
                    state_root: MerkleRoot::from_empty(),
                    height: service_context.get_current_height(),
                    tx_hash: stx.tx_hash.clone(),
                    cycles_used: service_context.get_cycles_used(),
                    events,
                    response: ReceiptResponse {
                        service_name: service_context.get_service_name().to_owned(),
                        method:       service_context.get_service_method().to_owned(),
                        response:     exec_resp,
                    },
                })
            })
            .collect::<Result<Vec<Receipt>, ProtocolError>>()?;

        self.hook(ctx.clone(), HookType::After, params)?;

        let state_root = self.commit(ctx)?;
        let mut all_cycles_used = 0;

        for receipt in receipts.iter_mut() {
            receipt.state_root = state_root.clone();
            all_cycles_used += receipt.cycles_used;
        }

        Ok(ExecutorResp {
            receipts,
            all_cycles_used,
            state_root,
        })
    }

    fn read(
        &self,
        params: &ExecutorParams,
        caller: &Address,
        cycles_price: u64,
        request: &TransactionRequest,
    ) -> ProtocolResult<ServiceResponse<String>> {
        let context = self.get_context(
            None,
            None,
            caller,
            cycles_price,
            std::u64::MAX,
            params,
            request,
        )?;
        panic::catch_unwind(AssertUnwindSafe(|| self.call(context, ExecType::Read)))
            .map_err(|e| ProtocolError::from(ExecutorError::QueryService(format!("{:?}", e))))
    }
}

impl<S: 'static + Storage, DB: 'static + TrieDB, Mapping: 'static + ServiceMapping> Dispatcher
    for ServiceExecutor<S, DB, Mapping>
{
    fn read(&self, context: ServiceContext) -> ServiceResponse<String> {
        self.call(context, ExecType::Read)
    }

    fn write(&self, context: ServiceContext) -> ServiceResponse<String> {
        self.call(context, ExecType::Write)
    }
}
