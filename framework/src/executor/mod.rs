mod factory;
#[cfg(test)]
mod tests;

pub use factory::ServiceExecutorFactory;

use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::Arc;

use cita_trie::DB as TrieDB;
use derive_more::{Display, From};

use protocol::traits::{
    Dispatcher, Executor, ExecutorParams, ExecutorResp, NoopDispatcher, ServiceMapping,
    ServiceResponse, ServiceState, Storage,
};
use protocol::types::{
    Address, ChainSchema, Hash, MerkleRoot, Receipt, ReceiptResponse, ServiceContext,
    ServiceContextParams, ServiceParam, ServiceSchema, SignedTransaction, TransactionRequest,
};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use crate::binding::state::{GeneralServiceState, MPTTrie};

enum HookType {
    Before,
    After,
}

#[derive(Clone)]
enum ExecType {
    Read,
    Write,
}

pub struct ServiceExecutor<S: Storage, DB: TrieDB, Mapping: ServiceMapping> {
    service_mapping: Arc<Mapping>,
    querier:         Rc<DefaultChainQuerier<S>>,
    states:          Rc<HashMap<String, Rc<RefCell<GeneralServiceState<DB>>>>>,
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
    pub async fn create_schema(
        trie_db: Arc<DB>,
        storage: Arc<S>,
        mapping: Arc<Mapping>,
    ) -> ProtocolResult<()> {
        let querier = Rc::new(DefaultChainQuerier::new(Arc::clone(&storage)));
        let mut schema = vec![];
        for name in mapping.list_service_name().iter() {
            let trie = MPTTrie::new(Arc::clone(&trie_db));
            let sdk = DefalutServiceSDK::new(
                Rc::new(RefCell::new(GeneralServiceState::new(trie))),
                Rc::clone(&querier),
                NoopDispatcher {},
            );
            let service = mapping.get_service(&name, sdk)?;
            let ret = service.schema_();
            schema.push(ServiceSchema {
                service: name.clone(),
                method:  ret.0,
                event:   ret.1,
            });
        }
        storage.insert_schema(ChainSchema { schema }).await
    }

    pub fn create_genesis(
        services: Vec<ServiceParam>,
        trie_db: Arc<DB>,
        storage: Arc<S>,
        mapping: Arc<Mapping>,
    ) -> ProtocolResult<MerkleRoot> {
        let querier = Rc::new(DefaultChainQuerier::new(Arc::clone(&storage)));

        let mut states = HashMap::new();
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
                DefalutServiceSDK::new(Rc::clone(state), Rc::clone(&querier), NoopDispatcher {});

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

        let mut states = HashMap::new();
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

    fn commit(&mut self) -> ProtocolResult<MerkleRoot> {
        for (name, state) in self.states.iter() {
            let root = state.borrow_mut().commit()?;
            self.root_state.borrow_mut().insert(name.to_owned(), root)?;
        }
        self.root_state.borrow_mut().stash()?;
        self.root_state.borrow_mut().commit()
    }

    fn stash(&mut self) -> ProtocolResult<()> {
        for state in self.states.values() {
            state.borrow_mut().stash()?;
        }

        Ok(())
    }

    fn revert_cache(&mut self) -> ProtocolResult<()> {
        for state in self.states.values() {
            state.borrow_mut().revert_cache()?;
        }

        Ok(())
    }

    fn hook(&mut self, hook: HookType, exec_params: &ExecutorParams) -> ProtocolResult<()> {
        for name in self.service_mapping.list_service_name().into_iter() {
            let sdk = self.get_sdk(&name)?;
            let mut service = self.service_mapping.get_service(name.as_str(), sdk)?;

            let hook_ret = match hook {
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
    ) -> ProtocolResult<DefalutServiceSDK<GeneralServiceState<DB>, DefaultChainQuerier<S>, Self>>
    {
        let state = self
            .states
            .get(service)
            .ok_or(ExecutorError::NotFoundService {
                service: service.to_owned(),
            })?;

        Ok(DefalutServiceSDK::new(
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

    fn catch_call(
        &mut self,
        context: ServiceContext,
        exec_type: ExecType,
    ) -> ProtocolResult<ServiceResponse<String>> {
        let result = match exec_type {
            ExecType::Read => panic::catch_unwind(AssertUnwindSafe(|| {
                self.call(context.clone(), exec_type.clone())
            })),
            ExecType::Write => panic::catch_unwind(AssertUnwindSafe(|| {
                self.call_with_tx_hooks(context.clone(), exec_type.clone())
            })),
        };
        match result {
            Ok(r) => {
                self.stash()?;
                if r.is_error() {
                    context.clear_events();
                }
                Ok(r)
            }
            Err(e) => {
                self.revert_cache()?;
                log::error!("inner chain error occurred when calling service: {:?}", e);
                Err(ExecutorError::CallService(format!("{:?}", e)).into())
            }
        }
    }

    fn call_with_tx_hooks(
        &self,
        context: ServiceContext,
        exec_type: ExecType,
    ) -> ServiceResponse<String> {
        let mut tx_hook_services = vec![];
        for name in self.service_mapping.list_service_name().into_iter() {
            let sdk = self
                .get_sdk(&name)
                .unwrap_or_else(|e| panic!("get target service sdk failed: {}", e));
            let tx_hook_service = self
                .service_mapping
                .get_service(name.as_str(), sdk)
                .unwrap_or_else(|e| panic!("get target service sdk failed: {}", e));
            tx_hook_services.push(tx_hook_service);
        }
        // TODO: If tx_hook_before_ failed, we should not exec the tx.
        // Need a mechanism for this.
        for tx_hook_service in tx_hook_services.iter_mut() {
            tx_hook_service.tx_hook_before_(context.clone());
        }
        let original_res = self.call(context.clone(), exec_type);
        // TODO: If the tx fails, status tx_hook_after_ changes will also be reverted.
        // It may not be what the developer want.
        // Need a new mechanism for this.
        for tx_hook_service in tx_hook_services.iter_mut() {
            tx_hook_service.tx_hook_after_(context.clone());
        }
        original_res
    }

    fn call(&self, context: ServiceContext, exec_type: ExecType) -> ServiceResponse<String> {
        let sdk = self
            .get_sdk(context.get_service_name())
            .unwrap_or_else(|e| panic!("get target service sdk failed: {}", e));

        let mut service = self
            .service_mapping
            .get_service(context.get_service_name(), sdk)
            .unwrap_or_else(|e| panic!("get target service failed: {}", e));

        match exec_type {
            ExecType::Read => service.read_(context),
            ExecType::Write => service.write_(context),
        }
    }
}

impl<S: 'static + Storage, DB: 'static + TrieDB, Mapping: 'static + ServiceMapping> Executor
    for ServiceExecutor<S, DB, Mapping>
{
    fn exec(
        &mut self,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp> {
        self.hook(HookType::Before, params)?;

        let mut receipts = txs
            .iter()
            .map(|stx| {
                let caller = Address::from_pubkey_bytes(stx.pubkey.clone())?;
                let context = self.get_context(
                    Some(stx.tx_hash.clone()),
                    Some(stx.raw.nonce.clone()),
                    &caller,
                    stx.raw.cycles_price,
                    stx.raw.cycles_limit,
                    params,
                    &stx.raw.request,
                )?;

                let exec_resp = self.catch_call(context.clone(), ExecType::Write)?;

                Ok(Receipt {
                    state_root:  MerkleRoot::from_empty(),
                    height:      context.get_current_height(),
                    tx_hash:     stx.tx_hash.clone(),
                    cycles_used: context.get_cycles_used(),
                    events:      context.get_events(),
                    response:    ReceiptResponse {
                        service_name: context.get_service_name().to_owned(),
                        method:       context.get_service_method().to_owned(),
                        response:     exec_resp,
                    },
                })
            })
            .collect::<Result<Vec<Receipt>, ProtocolError>>()?;

        self.hook(HookType::After, params)?;

        let state_root = self.commit()?;
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

#[derive(Debug, Display, From)]
pub enum ExecutorError {
    #[display(fmt = "service {:?} was not found", service)]
    NotFoundService { service: String },
    #[display(fmt = "service {:?} method {:?} was not found", service, method)]
    NotFoundMethod { service: String, method: String },
    #[display(fmt = "Parsing payload to json failed {:?}", _0)]
    JsonParse(serde_json::Error),

    #[display(fmt = "Init service genesis failed: {:?}", _0)]
    InitService(String),
    #[display(fmt = "Query service failed: {:?}", _0)]
    QueryService(String),
    #[display(fmt = "Call service failed: {:?}", _0)]
    CallService(String),
}

impl std::error::Error for ExecutorError {}

impl From<ExecutorError> for ProtocolError {
    fn from(err: ExecutorError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
