/// adapter use world state trie as data
use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;

use protocol::traits::executor::contract::ContractStateAdapter;
use protocol::traits::executor::{ContractSchema, ContractSer};
use protocol::types::UserAddress;
use protocol::ProtocolResult;

use crate::adapter::DexAdapter;
use crate::error::DexError;
use crate::types::{
    Config, ConfigsValue, FixedPendingOrderKey, FixedPendingOrderSchema, Order, OrderBook,
    OrderSide, OrderState, PendingOrders, TradingPair, TradingPairsValue, UserBalance, CONFIGS_KEY,
    FEE_ACCOUNT_KEY, TRADING_PAIRS_KEY,
};

struct FixedBytesSchema;
impl ContractSchema for FixedBytesSchema {
    type Key = FixedBytes;
    type Value = FixedBytes;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedBytes(pub Bytes);

impl ContractSer for FixedBytes {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(self.0.clone())
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(FixedBytes(bytes))
    }
}

pub struct NativeDexAdapter<StateAdapter: ContractStateAdapter> {
    state_adapter: Rc<RefCell<StateAdapter>>,
}

impl<StateAdapter: ContractStateAdapter> NativeDexAdapter<StateAdapter> {
    pub fn new(state_adapter: Rc<RefCell<StateAdapter>>) -> Self {
        Self { state_adapter }
    }

    fn set_order_in_orderbook(&mut self, order: Order) -> ProtocolResult<()> {
        let key = FixedPendingOrderKey {
            version:         order.version,
            trading_pair_id: order.trading_pair_id,
        };
        let mut pending_orders: PendingOrders = self
            .state_adapter
            .borrow()
            .get::<FixedPendingOrderSchema>(&key)?
            .ok_or_else(|| DexError::Adapter("invalid pending order data".to_string()))?;
        if order.state == OrderState::Pending {
            pending_orders.inner.insert(order.id.clone(), order);
        } else {
            pending_orders.inner.remove(&order.id);
        }
        self.state_adapter
            .borrow_mut()
            .insert_cache::<FixedPendingOrderSchema>(key, pending_orders)
    }
}

impl<StateAdapter: ContractStateAdapter> DexAdapter for NativeDexAdapter<StateAdapter> {
    fn get_fee_account(&self) -> ProtocolResult<UserAddress> {
        let value = self
            .state_adapter
            .borrow()
            .get::<FixedBytesSchema>(&FixedBytes(Bytes::from(FEE_ACCOUNT_KEY)))?
            .ok_or_else(|| DexError::Adapter("invalid fee_account data".to_string()))?;
        UserAddress::from_bytes(value.0)
    }

    fn update_fee_account(&mut self, new_account: UserAddress) -> ProtocolResult<()> {
        self.state_adapter
            .borrow_mut()
            .insert_cache::<FixedBytesSchema>(
                FixedBytes(Bytes::from(FEE_ACCOUNT_KEY)),
                FixedBytes(new_account.as_bytes()),
            )
    }

    fn get_trading_pairs(&self) -> ProtocolResult<Vec<TradingPair>> {
        let option_value = self
            .state_adapter
            .borrow()
            .get::<FixedBytesSchema>(&FixedBytes(Bytes::from(TRADING_PAIRS_KEY)))?;
        match option_value {
            None => Ok(vec![]),
            Some(value) => Ok(rlp::decode::<TradingPairsValue>(&value.0)
                .map_err(|_| DexError::Adapter("invalid trading_pairs data".to_string()))?
                .0),
        }
    }

    fn add_trading_pair(&mut self, pair: TradingPair) -> ProtocolResult<u64> {
        let option_value = self
            .state_adapter
            .borrow()
            .get::<FixedBytesSchema>(&FixedBytes(Bytes::from(TRADING_PAIRS_KEY)))?;
        let mut pairs = match option_value {
            None => TradingPairsValue(vec![]),
            Some(value) => rlp::decode::<TradingPairsValue>(&value.0)
                .map_err(|_| DexError::Adapter("invalid trading_pairs data".to_string()))?,
        };
        pairs.0.push(pair);
        let return_value = pairs.0.len() as u64;
        self.state_adapter
            .borrow_mut()
            .insert_cache::<FixedBytesSchema>(
                FixedBytes(Bytes::from(TRADING_PAIRS_KEY)),
                FixedBytes(Bytes::from(rlp::encode(&pairs))),
            )?;
        Ok(return_value)
    }

    fn new_config(&mut self, config: Config) -> ProtocolResult<u64> {
        let option_value = self
            .state_adapter
            .borrow()
            .get::<FixedBytesSchema>(&FixedBytes(Bytes::from(CONFIGS_KEY)))?;
        let mut configs = match option_value {
            None => ConfigsValue(vec![]),
            Some(value) => rlp::decode::<ConfigsValue>(&value.0)
                .map_err(|_| DexError::Adapter("invalid configs data".to_string()))?,
        };
        configs.0.push(config);
        let return_value = configs.0.len() as u64;
        self.state_adapter
            .borrow_mut()
            .insert_cache::<FixedBytesSchema>(
                FixedBytes(Bytes::from(CONFIGS_KEY)),
                FixedBytes(Bytes::from(rlp::encode(&configs))),
            )?;
        Ok(return_value)
    }

    fn get_configs(&self) -> ProtocolResult<Vec<Config>> {
        let option_value = self
            .state_adapter
            .borrow()
            .get::<FixedBytesSchema>(&FixedBytes(Bytes::from(CONFIGS_KEY)))?;
        match option_value {
            None => Ok(vec![]),
            Some(value) => Ok(rlp::decode::<ConfigsValue>(&value.0)
                .map_err(|_| DexError::Adapter("invalid configs data".to_string()))?
                .0),
        }
    }

    fn get_balance(&self, user: &UserAddress) -> ProtocolResult<UserBalance> {
        // TODO: add BALANCES_KEY_PREFIX to the key
        let option_value = self
            .state_adapter
            .borrow()
            .get::<FixedBytesSchema>(&FixedBytes(user.as_bytes()))?;
        match option_value {
            None => Ok(UserBalance::default()),
            Some(value) => Ok(rlp::decode::<UserBalance>(&value.0)
                .map_err(|_| DexError::Adapter("invalid balance data".to_string()))?),
        }
    }

    fn set_balance(&mut self, user: UserAddress, balances: UserBalance) -> ProtocolResult<()> {
        self.state_adapter
            .borrow_mut()
            .insert_cache::<FixedBytesSchema>(
                FixedBytes(user.as_bytes()),
                FixedBytes(Bytes::from(rlp::encode(&balances))),
            )
    }

    fn get_order(&self, id: &str) -> ProtocolResult<Option<Order>> {
        // TODO: add ORDERS_KEY_PREFIX to the key
        let option_value = self
            .state_adapter
            .borrow()
            .get::<FixedBytesSchema>(&FixedBytes(Bytes::from(id)))?;

        match option_value {
            None => Ok(None),
            Some(value) => {
                Ok(Some(rlp::decode::<Order>(&value.0).map_err(|_| {
                    DexError::Adapter("invalid order data".to_string())
                })?))
            }
        }
    }

    fn set_order(&mut self, order: Order) -> ProtocolResult<String> {
        self.state_adapter
            .borrow_mut()
            .insert_cache::<FixedBytesSchema>(
                FixedBytes(Bytes::from(order.id.clone())),
                FixedBytes(Bytes::from(rlp::encode(&order))),
            )?;
        self.set_order_in_orderbook(order.clone())?;
        Ok(order.id.clone())
    }

    fn get_pending_orders(
        &self,
        version: u64,
        trading_pair_id: u64,
        user: &UserAddress,
    ) -> ProtocolResult<Vec<Order>> {
        let key = FixedPendingOrderKey {
            version,
            trading_pair_id,
        };
        let pending_orders: PendingOrders = self
            .state_adapter
            .borrow()
            .get::<FixedPendingOrderSchema>(&key)?
            .unwrap_or_default();
        let orders = pending_orders
            .inner
            .into_iter()
            .map(|(_, o)| o)
            .filter(|o| {
                o.version == version
                    && o.trading_pair_id == trading_pair_id
                    && o.state == OrderState::Pending
                    && &o.user == user
            })
            .collect::<Vec<_>>();
        Ok(orders)
    }

    fn get_orderbook(&self, version: u64, trading_pair_id: u64) -> ProtocolResult<OrderBook> {
        let key = FixedPendingOrderKey {
            version,
            trading_pair_id,
        };
        let pending_orders: PendingOrders = self
            .state_adapter
            .borrow()
            .get::<FixedPendingOrderSchema>(&key)?
            .unwrap_or_default();

        let mut order_book = OrderBook {
            version,
            trading_pair_id,
            ..Default::default()
        };
        for order in pending_orders
            .inner
            .into_iter()
            .map(|(_, o)| o)
            .filter(|o| {
                o.version == version
                    && o.trading_pair_id == trading_pair_id
                    && o.state == OrderState::Pending
            })
        {
            match order.order_side {
                OrderSide::Buy => order_book.buy_orders.push(order.clone()),
                OrderSide::Sell => order_book.sell_orders.push(order.clone()),
            }
        }
        order_book
            .buy_orders
            .sort_by(|a, b| a.price.cmp(&b.price).reverse());
        order_book.sell_orders.sort_by(|a, b| a.price.cmp(&b.price));
        Ok(order_book)
    }
}
