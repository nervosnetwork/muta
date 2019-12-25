#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use cita_trie::DB as TrieDB;
use derive_more::{Display, From};

use asset::AssetService;
use bytes::Bytes;
use protocol::traits::{
    ExecResp, Executor, ExecutorParams, ExecutorResp, RequestContext, Service, ServiceState,
    Storage,
};
use protocol::types::{
    Address, Bloom, BloomInput, GenesisService, Hash, MerkleRoot, Receipt, ReceiptResponse,
    SignedTransaction, TransactionRequest,
};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use crate::binding::state::{GeneralServiceState, MPTTrie};
use crate::{ContextParams, DefaultRequestContext};

pub struct ServiceExecutor<S: Storage, DB: TrieDB> {
    querier:    Rc<DefaultChainQuerier<S>>,
    states:     HashMap<String, Rc<RefCell<GeneralServiceState<DB>>>>,
    root_state: GeneralServiceState<DB>,
}

impl<S: Storage, DB: 'static + TrieDB> ServiceExecutor<S, DB> {
    pub fn create_genesis(
        genesis_services: Vec<GenesisService>,
        trie_db: Arc<DB>,
        storage: Arc<S>,
    ) -> ProtocolResult<MerkleRoot> {
        let querier = Rc::new(DefaultChainQuerier::new(Arc::clone(&storage)));

        let mut states = HashMap::new();
        for service_alloc in genesis_services.iter() {
            let trie = MPTTrie::new(Arc::clone(&trie_db));

            states.insert(
                service_alloc.service.to_owned(),
                Rc::new(RefCell::new(GeneralServiceState::new(trie))),
            );
        }

        for service_alloc in genesis_services.into_iter() {
            let ctx_params = ContextParams {
                cycles_limit:    std::u64::MAX,
                cycles_price:    1,
                cycles_used:     Rc::new(RefCell::new(0)),
                caller:          Address::from_hex(&service_alloc.caller)?,
                epoch_id:        0,
                timestamp:       0,
                service_name:    service_alloc.service.to_owned(),
                service_method:  service_alloc.method.to_owned(),
                service_payload: service_alloc.payload.to_owned(),
                events:          Rc::new(RefCell::new(vec![])),
            };

            let context = DefaultRequestContext::new(ctx_params);
            let state =
                states
                    .get(context.get_service_name())
                    .ok_or(ExecutorError::NotFoundService {
                        service: context.get_service_name().to_owned(),
                    })?;
            let sdk =
                DefalutServiceSDK::new(Rc::clone(state), Rc::clone(&querier), context.clone());

            match context.get_service_name() {
                "asset" => {
                    let mut service = AssetService::init_(sdk)?;
                    service.write_(context.clone())?;
                }
                _ => unreachable!(),
            };

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

    pub fn with_root(root: MerkleRoot, trie_db: Arc<DB>, storage: Arc<S>) -> ProtocolResult<Self> {
        let trie = MPTTrie::from(root.clone(), Arc::clone(&trie_db))?;
        let root_state = GeneralServiceState::new(trie);

        let asset_root =
            root_state
                .get(&"asset".to_owned())?
                .ok_or(ExecutorError::NotFoundService {
                    service: "asset".to_owned(),
                })?;
        let trie = MPTTrie::from(asset_root, Arc::clone(&trie_db))?;
        let asset_state = GeneralServiceState::new(trie);

        let mut states = HashMap::new();
        states.insert("asset".to_owned(), Rc::new(RefCell::new(asset_state)));

        Ok(Self {
            querier: Rc::new(DefaultChainQuerier::new(storage)),
            states,
            root_state,
        })
    }

    fn commit(&mut self) -> ProtocolResult<MerkleRoot> {
        for (name, state) in self.states.iter() {
            let root = state.borrow_mut().commit()?;
            self.root_state.insert(name.to_owned(), root)?;
        }
        self.root_state.stash()?;
        self.root_state.commit()
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

    fn get_sdk(
        &self,
        context: DefaultRequestContext,
        service: &str,
    ) -> ProtocolResult<
        DefalutServiceSDK<GeneralServiceState<DB>, DefaultChainQuerier<S>, DefaultRequestContext>,
    > {
        let state = self
            .states
            .get(service)
            .ok_or(ExecutorError::NotFoundService {
                service: service.to_owned(),
            })?;

        Ok(DefalutServiceSDK::new(
            Rc::clone(&state),
            Rc::clone(&self.querier),
            context,
        ))
    }

    fn get_context(
        &self,
        params: &ExecutorParams,
        caller: &Address,
        cycles_price: u64,
        request: &TransactionRequest,
    ) -> ProtocolResult<DefaultRequestContext> {
        let ctx_params = ContextParams {
            cycles_limit: params.cycels_limit,
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

        Ok(DefaultRequestContext::new(ctx_params))
    }

    fn exec_service(
        &self,
        context: DefaultRequestContext,
        readonly: bool,
    ) -> ProtocolResult<ExecResp> {
        let sdk = self.get_sdk(context.clone(), context.get_service_name())?;

        let mut service = match context.get_service_name() {
            "asset" => AssetService::init_(sdk)?,
            _ => {
                return Err(ExecutorError::NotFoundService {
                    service: context.get_service_name().to_owned(),
                }
                .into())
            }
        };

        let result = if readonly {
            service.read_(context.clone())
        } else {
            service.write_(context.clone())
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
                let bytes = Bytes::from((event.service.clone() + &event.data).as_bytes());
                let hash = Hash::digest(bytes).as_bytes();

                let input = BloomInput::Raw(hash.as_ref());
                bloom.accrue(input)
            }
        }

        bloom
    }
}

impl<S: Storage, DB: 'static + TrieDB> Executor for ServiceExecutor<S, DB> {
    fn exec(
        &mut self,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp> {
        let mut receipts = txs
            .iter()
            .map(|stx| {
                let caller = Address::from_pubkey_bytes(stx.pubkey.clone())?;
                let context =
                    self.get_context(params, &caller, stx.raw.cycles_price, &stx.raw.request)?;

                let exec_resp = self.exec_service(context.clone(), false)?;

                if exec_resp.is_error {
                    self.stash()?;
                } else {
                    self.revert_cache()?;
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
        self.exec_service(context, true)
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
