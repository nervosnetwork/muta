mod error;
mod factory;
#[cfg(test)]
mod tests;

pub use factory::ServiceExecutorFactory;

use std::{
    cell::RefCell,
    collections::HashMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    panic::{self, AssertUnwindSafe},
    rc::Rc,
    sync::Arc,
};

use cita_trie::DB as TrieDB;

use common_apm::muta_apm;
use protocol::traits::{
    Context, Executor, ExecutorParams, ExecutorResp, Service, ServiceMapping, ServiceResponse,
    ServiceState, Storage,
};
use protocol::types::{
    Address, Event, Hash, MerkleRoot, Receipt, ReceiptResponse, ServiceContext,
    ServiceContextParams, ServiceParam, SignedTransaction, TransactionRequest,
};
use protocol::{ProtocolError, ProtocolResult};

use crate::binding::sdk::{DefaultChainQuerier, DefaultSDKFactory};
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

pub struct ServiceStateMap<DB: TrieDB>(HashMap<String, Rc<RefCell<GeneralServiceState<DB>>>>);

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
    inner:  Vec<Rc<RefCell<Box<dyn Service>>>>,
    states: Rc<ServiceStateMap<DB>>,
}

impl<DB: TrieDB> CommitHooks<DB> {
    fn new(
        hooks: Vec<Rc<RefCell<Box<dyn Service>>>>,
        states: Rc<ServiceStateMap<DB>>,
    ) -> CommitHooks<DB> {
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
                hook.borrow_mut().tx_hook_before_(service_context.clone())
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
                hook.borrow_mut().tx_hook_after_(service_context.clone())
            })?;
            ret.push(resp);
        }

        Ok(ret)
    }
}

pub struct ServiceExecutor<S: Storage, DB: TrieDB, Mapping: ServiceMapping> {
    service_mapping: Arc<Mapping>,
    states:          Rc<ServiceStateMap<DB>>,
    root_state:      GeneralServiceState<DB>,
    services:        HashMap<String, Rc<RefCell<Box<dyn Service>>>>,

    phantom: PhantomData<S>,
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

        let states = Rc::new(states);
        let sdk_factory = DefaultSDKFactory::new(Rc::clone(&states), Rc::clone(&querier));

        for params in services.into_iter() {
            let state = states
                .get(&params.name)
                .ok_or(ExecutorError::NotFoundService {
                    service: params.name.to_owned(),
                })?;

            let mut service = mapping.get_service(&params.name, &sdk_factory)?;
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
        let querier = Rc::new(DefaultChainQuerier::new(Arc::clone(&storage)));
        let trie = MPTTrie::from(root, Arc::clone(&trie_db))?;
        let root_state = GeneralServiceState::new(trie);
        let list_service_name = service_mapping.list_service_name();

        let mut states = ServiceStateMap::new();
        for name in list_service_name.iter() {
            let trie = match root_state.get(name)? {
                Some(service_root) => MPTTrie::from(service_root, Arc::clone(&trie_db))?,
                None => MPTTrie::new(Arc::clone(&trie_db)),
            };

            let service_state = GeneralServiceState::new(trie);
            states.insert(name.to_owned(), Rc::new(RefCell::new(service_state)));
        }

        let states = Rc::new(states);
        let sdk_factory = DefaultSDKFactory::new(Rc::clone(&states), Rc::clone(&querier));

        let mut services = HashMap::new();
        for name in list_service_name.iter() {
            let service = service_mapping.get_service(name, &sdk_factory)?;
            services.insert(name.clone(), Rc::new(RefCell::new(service)));
        }

        Ok(Self {
            service_mapping,
            states,
            root_state,
            services,
            phantom: PhantomData,
        })
    }

    #[muta_apm::derive::tracing_span(kind = "executor.commit")]
    fn commit(&mut self, ctx: Context) -> ProtocolResult<MerkleRoot> {
        for (name, state) in self.states.iter() {
            let root = state.borrow_mut().commit()?;
            self.root_state.insert(name.to_owned(), root)?;
        }
        self.root_state.stash()?;
        self.root_state.commit()
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
            let service = self.get_service(name.as_str())?;

            let hook_ret = match hook_type {
                HookType::Before => panic::catch_unwind(AssertUnwindSafe(|| {
                    service.borrow_mut().hook_before_(exec_params)
                })),
                HookType::After => panic::catch_unwind(AssertUnwindSafe(|| {
                    service.borrow_mut().hook_after_(exec_params)
                })),
            };

            if hook_ret.is_err() {
                self.revert_cache()?;
            } else {
                self.stash()?;
            }
        }
        Ok(())
    }

    fn get_service(&self, service: &str) -> ProtocolResult<Rc<RefCell<Box<dyn Service>>>> {
        self.services
            .get(service)
            .map(|s| Rc::clone(s))
            .ok_or_else(|| {
                ExecutorError::NotFoundService {
                    service: service.to_owned(),
                }
                .into()
            })
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
        event: Rc<RefCell<Vec<Event>>>,
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
            events: event,
        };

        Ok(ServiceContext::new(ctx_params))
    }

    fn get_tx_hooks(&self, exec_type: ExecType) -> Box<dyn TxHooks> {
        match exec_type {
            ExecType::Read => Box::new(()),
            ExecType::Write => {
                let mut tx_hooks = vec![];

                for name in self.service_mapping.list_service_name().into_iter() {
                    let tx_hook_service = self.get_service(name.as_str()).expect("no service");
                    tx_hooks.push(tx_hook_service);
                }

                Box::new(CommitHooks::new(tx_hooks, Rc::clone(&self.states)))
            }
        }
    }

    fn catch_call(
        &mut self,
        context: Context,
        service_context: ServiceContext,
        exec_type: ExecType,
        event: Rc<RefCell<Vec<Event>>>,
    ) -> ProtocolResult<ServiceResponse<String>> {
        let mut tx_hooks = self.get_tx_hooks(exec_type);

        let resp = tx_hooks.before(context.clone(), service_context.clone())?;
        self.states.stash()?;

        let event_index = event.borrow_mut().len();

        let ret = if resp.iter().any(|r| r.is_error()) {
            self.revert_cache()?;
            event.borrow_mut().truncate(event_index);
            ServiceResponse::from_error(65535, "skip_tx_run".to_owned())
        } else {
            match panic::catch_unwind(AssertUnwindSafe(|| {
                self.call(service_context.clone(), exec_type)
            })) {
                Ok(r) => Ok(r),
                Err(e) => {
                    self.revert_cache()?;
                    log::error!("inner chain error occurred when calling service: {:?}", e);
                    Err(ProtocolError::from(ExecutorError::CallService(format!(
                        "{:?}",
                        e
                    ))))
                }
            }?
        };

        if ret.is_error() {
            service_context.cancel("tx_exec_return_code_not_zero".to_owned());
        }

        let resp = tx_hooks.after(context, service_context)?;

        if resp.iter().any(|r| r.is_error()) {
            event.borrow_mut().truncate(event_index);
            self.states.revert_cache()?;
        } else {
            self.states.stash()?;
        }

        Ok(ret)
    }

    fn call(&self, context: ServiceContext, exec_type: ExecType) -> ServiceResponse<String> {
        let service_name = context.get_service_name();
        let service = self.get_service(service_name);

        if service.is_err() {
            return ServiceResponse::from_error(
                SERVICE_NOT_FOUND_CODE,
                "can not found service".to_owned(),
            );
        }

        let service = service.unwrap();
        match exec_type {
            ExecType::Read => service.borrow().read_(context),
            ExecType::Write => service.borrow_mut().write_(context),
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
                let event = Rc::new(RefCell::new(vec![]));
                let service_context = self.get_context(
                    Some(stx.tx_hash.clone()),
                    Some(stx.raw.nonce.clone()),
                    &stx.raw.sender,
                    stx.raw.cycles_price,
                    stx.raw.cycles_limit,
                    params,
                    &stx.raw.request,
                    Rc::clone(&event),
                )?;

                let exec_resp = self.catch_call(
                    ctx.clone(),
                    service_context.clone(),
                    ExecType::Write,
                    Rc::clone(&event),
                )?;
                Ok(Receipt {
                    state_root:  MerkleRoot::from_empty(),
                    height:      service_context.get_current_height(),
                    tx_hash:     stx.tx_hash.clone(),
                    cycles_used: service_context.get_cycles_used(),
                    events:      service_context.get_events(),
                    response:    ReceiptResponse {
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
            Rc::new(RefCell::new(vec![])),
        )?;
        panic::catch_unwind(AssertUnwindSafe(|| self.call(context, ExecType::Read)))
            .map_err(|e| ProtocolError::from(ExecutorError::QueryService(format!("{:?}", e))))
    }
}
