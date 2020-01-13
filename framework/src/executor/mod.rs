mod factory;
#[cfg(test)]
mod tests;

pub use factory::ServiceExecutorFactory;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use cita_trie::DB as TrieDB;
use derive_more::{Display, From};

use bytes::BytesMut;
use protocol::traits::{
    Dispatcher, ExecResp, Executor, ExecutorParams, ExecutorResp, NoopDispatcher, ServiceMapping,
    ServiceState, Storage,
};
use protocol::types::{
    Address, Bloom, BloomInput, Hash, MerkleRoot, Receipt, ReceiptResponse, ServiceContext,
    ServiceContextParams, ServiceParam, SignedTransaction, TransactionRequest,
};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use crate::binding::state::{GeneralServiceState, MPTTrie};

enum HookType {
    Before,
    After,
}

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
            service.genesis_(params.payload.clone())?;

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

    fn hook(&mut self, hook: HookType) -> ProtocolResult<()> {
        for name in self.service_mapping.list_service_name().into_iter() {
            let state = self
                .states
                .get(&name)
                .ok_or(ExecutorError::NotFoundService {
                    service: name.to_owned(),
                })?;

            let sdk = self.get_sdk(&name)?;
            let mut service = self.service_mapping.get_service(name.as_str(), sdk)?;

            match hook {
                HookType::Before => service.hook_before_()?,
                HookType::After => service.hook_after_()?,
            };

            state.borrow_mut().stash()?;
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
        params: &ExecutorParams,
        caller: &Address,
        cycles_price: u64,
        request: &TransactionRequest,
    ) -> ProtocolResult<ServiceContext> {
        let ctx_params = ServiceContextParams {
            cycles_limit: params.cycles_limit,
            cycles_price,
            cycles_used: Rc::new(RefCell::new(0)),
            caller: caller.clone(),
            epoch_id: params.epoch_id,
            timestamp: params.timestamp,
            service_name: request.service_name.to_owned(),
            service_method: request.method.to_owned(),
            service_payload: request.payload.to_owned(),
            events: Rc::new(RefCell::new(vec![])),
        };

        Ok(ServiceContext::new(ctx_params))
    }

    fn catch_call(&self, context: ServiceContext, exec_type: ExecType) -> ExecResp {
        match self.call(context, exec_type) {
            Ok(resp) => resp,
            Err(e) => ExecResp {
                ret:      e.to_string(),
                is_error: true,
            },
        }
    }

    fn call(&self, context: ServiceContext, exec_type: ExecType) -> ProtocolResult<ExecResp> {
        let sdk = self.get_sdk(context.get_service_name())?;

        let mut service = self
            .service_mapping
            .get_service(context.get_service_name(), sdk)?;

        let result = match exec_type {
            ExecType::Read => service.read_(context),
            ExecType::Write => service.write_(context),
        };

        let (ret, is_error) = match result {
            Ok(ret) => (ret, false),
            Err(e) => (e.to_string(), true),
        };

        Ok(ExecResp { ret, is_error })
    }

    fn logs_bloom(&self, receipts: &[Receipt]) -> Bloom {
        let mut bloom = Bloom::default();
        for receipt in receipts {
            for event in receipt.events.iter() {
                let bytes =
                    BytesMut::from((event.service.clone() + &event.data).as_bytes()).freeze();
                let hash = Hash::digest(bytes).as_bytes();

                let input = BloomInput::Raw(hash.as_ref());
                bloom.accrue(input)
            }
        }

        bloom
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
        self.hook(HookType::Before)?;

        let mut receipts = txs
            .iter()
            .map(|stx| {
                let caller = Address::from_pubkey_bytes(stx.pubkey.clone())?;
                let context =
                    self.get_context(params, &caller, stx.raw.cycles_price, &stx.raw.request)?;

                let exec_resp = self.catch_call(context.clone(), ExecType::Write);

                if exec_resp.is_error {
                    self.revert_cache()?;
                } else {
                    self.stash()?;
                };

                Ok(Receipt {
                    state_root:  MerkleRoot::from_empty(),
                    epoch_id:    context.get_current_epoch_id(),
                    tx_hash:     stx.tx_hash.clone(),
                    cycles_used: context.get_cycles_used(),
                    events:      context.get_events(),
                    response:    ReceiptResponse {
                        service_name: context.get_service_name().to_owned(),
                        method:       context.get_service_method().to_owned(),
                        ret:          exec_resp.ret,
                        is_error:     exec_resp.is_error,
                    },
                })
            })
            .collect::<Result<Vec<Receipt>, ProtocolError>>()?;

        self.hook(HookType::After)?;

        let state_root = self.commit()?;
        let mut all_cycles_used = 0;

        for receipt in receipts.iter_mut() {
            receipt.state_root = state_root.clone();
            all_cycles_used += receipt.cycles_used;
        }
        let logs_bloom = self.logs_bloom(&receipts);

        Ok(ExecutorResp {
            receipts,
            all_cycles_used,
            state_root,
            logs_bloom,
        })
    }

    fn read(
        &self,
        params: &ExecutorParams,
        caller: &Address,
        cycles_price: u64,
        request: &TransactionRequest,
    ) -> ProtocolResult<ExecResp> {
        let context = self.get_context(params, caller, cycles_price, request)?;
        self.call(context, ExecType::Read)
    }
}

impl<S: 'static + Storage, DB: 'static + TrieDB, Mapping: 'static + ServiceMapping> Dispatcher
    for ServiceExecutor<S, DB, Mapping>
{
    fn read(&self, context: ServiceContext) -> ProtocolResult<ExecResp> {
        self.call(context, ExecType::Read)
    }

    fn write(&self, context: ServiceContext) -> ProtocolResult<ExecResp> {
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
}
impl std::error::Error for ExecutorError {}

impl From<ExecutorError> for ProtocolError {
    fn from(err: ExecutorError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
