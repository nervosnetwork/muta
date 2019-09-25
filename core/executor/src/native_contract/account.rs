use std::cell::RefCell;
use std::collections::BTreeMap;
use std::error::Error;
use std::rc::Rc;

use derive_more::{Display, From};

use protocol::traits::executor::contract::{AccountContract, ContractStateAdapter};
use protocol::traits::executor::RcInvokeContext;
use protocol::types::{
    Account, Address, AssetID, AssetInfo, Balance, ContractAccount, Hash, UserAccount,
};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::cycles::{consume_cycles, CyclesAction};
use crate::fixed_types::{FixedAccount, FixedAccountSchema, FixedAddress};

pub struct NativeAccountContract<StateAdapter: ContractStateAdapter> {
    state_adapter: Rc<RefCell<StateAdapter>>,
}

impl<StateAdapter: ContractStateAdapter> NativeAccountContract<StateAdapter> {
    pub fn new(state_adapter: Rc<RefCell<StateAdapter>>) -> Self {
        Self { state_adapter }
    }
}

impl<StateAdapter: ContractStateAdapter> AccountContract<StateAdapter>
    for NativeAccountContract<StateAdapter>
{
    fn transfer(&mut self, ictx: RcInvokeContext, to: &Address) -> ProtocolResult<()> {
        let carrying_asset = ictx
            .borrow()
            .carrying_asset
            .clone()
            .ok_or(NativeAccountContractError::InsufficientBalance)?;

        self.sub_balance(
            &carrying_asset.asset_id,
            &ictx.borrow().caller,
            carrying_asset.amount.clone(),
        )?;
        self.add_balance(&carrying_asset.asset_id, to, carrying_asset.amount.clone())?;

        let mut fee = ictx.borrow().cycles_used.clone();
        consume_cycles(
            CyclesAction::AccountTransfer,
            ictx.borrow().cycles_price,
            &mut fee,
            &ictx.borrow().cycles_limit,
        )?;
        ictx.borrow_mut().cycles_used = fee;
        Ok(())
    }

    fn create_account(&mut self, address: &Address) -> ProtocolResult<Account> {
        self.find_or_create(address)
    }

    fn add_balance(
        &mut self,
        id: &AssetID,
        address: &Address,
        amount: Balance,
    ) -> ProtocolResult<()> {
        let account = self.find_or_create(address)?;

        let modified_account = match account {
            Account::User(mut user) => {
                self.add_balance_with_user(id, &mut user, amount)?;
                Account::User(user)
            }
            Account::Contract(mut contract) => {
                self.add_balance_with_contract(id, &mut contract, amount)?;
                Account::Contract(contract)
            }
        };

        self.state_adapter
            .borrow_mut()
            .insert_cache::<FixedAccountSchema>(
                FixedAddress::new(address.clone()),
                FixedAccount::new(modified_account),
            )?;
        Ok(())
    }

    fn sub_balance(
        &mut self,
        id: &AssetID,
        address: &Address,
        amount: Balance,
    ) -> ProtocolResult<()> {
        let account = self.find_or_create(address)?;

        let modified_account = match account {
            Account::User(mut user) => {
                self.sub_balance_with_user(id, &mut user, amount)?;
                Account::User(user)
            }
            Account::Contract(mut contract) => {
                self.sub_balance_with_contract(id, &mut contract, amount)?;
                Account::Contract(contract)
            }
        };

        self.state_adapter
            .borrow_mut()
            .insert_cache::<FixedAccountSchema>(
                FixedAddress::new(address.clone()),
                FixedAccount::new(modified_account),
            )?;
        Ok(())
    }

    fn inc_nonce(&mut self, ictx: RcInvokeContext) -> ProtocolResult<()> {
        let caller = &ictx.borrow().caller;
        let account = self.get_account(caller)?;

        let modified_account = match account {
            Account::User(user) => Account::User(UserAccount {
                nonce:  user.nonce + 1,
                assets: user.assets,
            }),
            Account::Contract(contract) => Account::Contract(ContractAccount {
                nonce:        contract.nonce + 1,
                assets:       contract.assets,
                storage_root: contract.storage_root,
            }),
        };

        self.state_adapter
            .borrow_mut()
            .insert_cache::<FixedAccountSchema>(
                FixedAddress::new(caller.clone()),
                FixedAccount::new(modified_account),
            )?;
        Ok(())
    }

    fn get_balance(&self, id: &AssetID, address: &Address) -> ProtocolResult<Balance> {
        let fixed_account = self
            .state_adapter
            .borrow()
            .get::<FixedAccountSchema>(&FixedAddress::new(address.clone()))?
            .ok_or(NativeAccountContractError::AccountNotFound {
                address: address.clone(),
            })?;

        match fixed_account.inner {
            Account::User(user) => self.get_balance_with_user(id, &user),
            Account::Contract(contract) => self.get_balance_with_contract(id, &contract),
        }
    }

    fn get_account(&self, address: &Address) -> ProtocolResult<Account> {
        let fixed_accoount = self
            .state_adapter
            .borrow()
            .get::<FixedAccountSchema>(&FixedAddress::new(address.clone()))?
            .ok_or(NativeAccountContractError::AccountNotFound {
                address: address.clone(),
            })?;
        Ok(fixed_accoount.inner)
    }

    fn get_nonce(&self, address: &Address) -> ProtocolResult<u64> {
        let account = self.get_account(address)?;

        match account {
            Account::User(user) => Ok(user.nonce),
            Account::Contract(contract) => Ok(contract.nonce),
        }
    }
}

impl<StateAdapter: ContractStateAdapter> NativeAccountContract<StateAdapter> {
    fn find_or_create(&self, address: &Address) -> ProtocolResult<Account> {
        if let Some(fixed_account) = self
            .state_adapter
            .borrow()
            .get::<FixedAccountSchema>(&FixedAddress::new(address.clone()))?
        {
            return Ok(fixed_account.inner);
        }

        let account = match address {
            Address::User(_) => Account::User(UserAccount {
                nonce:  0,
                assets: BTreeMap::new(),
            }),
            Address::Contract(_) => Account::Contract(ContractAccount {
                nonce:        0,
                assets:       BTreeMap::new(),
                storage_root: Hash::from_empty(),
            }),
        };
        Ok(account)
    }

    fn get_balance_with_user(
        &self,
        id: &AssetID,
        account: &UserAccount,
    ) -> ProtocolResult<Balance> {
        if let Some(info) = account.assets.get(id) {
            Ok(info.balance.clone())
        } else {
            Ok(Balance::from(0u64))
        }
    }

    fn get_balance_with_contract(
        &self,
        id: &AssetID,
        account: &ContractAccount,
    ) -> ProtocolResult<Balance> {
        if let Some(balance) = account.assets.get(id) {
            Ok(balance.clone())
        } else {
            Ok(Balance::from(0u64))
        }
    }

    fn add_balance_with_user(
        &self,
        id: &AssetID,
        account: &mut UserAccount,
        amount: Balance,
    ) -> ProtocolResult<()> {
        account
            .assets
            .entry(id.clone())
            .and_modify(|info| info.balance += amount.clone())
            .or_insert_with(|| AssetInfo {
                balance:  amount,
                approved: BTreeMap::new(),
            });
        Ok(())
    }

    fn add_balance_with_contract(
        &mut self,
        id: &AssetID,
        account: &mut ContractAccount,
        amount: Balance,
    ) -> ProtocolResult<()> {
        account
            .assets
            .entry(id.clone())
            .and_modify(|balance| *balance += amount.clone())
            .or_insert(amount);
        Ok(())
    }

    fn sub_balance_with_user(
        &mut self,
        id: &AssetID,
        account: &mut UserAccount,
        amount: Balance,
    ) -> ProtocolResult<()> {
        if let Some(info) = account.assets.get_mut(id) {
            if info.balance < amount {
                return Err(NativeAccountContractError::InsufficientBalance.into());
            }

            info.balance -= amount;
            return Ok(());
        }

        Err(NativeAccountContractError::InsufficientBalance.into())
    }

    fn sub_balance_with_contract(
        &self,
        id: &AssetID,
        account: &mut ContractAccount,
        amount: Balance,
    ) -> ProtocolResult<()> {
        account
            .assets
            .entry(id.clone())
            .and_modify(|balance| *balance -= amount.clone())
            .or_insert(amount);
        Ok(())
    }
}

#[derive(Debug, Display, From)]
pub enum NativeAccountContractError {
    #[display(fmt = "Insufficient balance")]
    InsufficientBalance,

    #[display(fmt = "account {:?} not found", address)]
    AccountNotFound { address: Address },

    #[display(fmt = "invalid address")]
    InvalidAddress,

    #[display(fmt = "fixed codec {:?}", _0)]
    FixedCodec(rlp::DecoderError),
}

impl Error for NativeAccountContractError {}

impl From<NativeAccountContractError> for ProtocolError {
    fn from(err: NativeAccountContractError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
