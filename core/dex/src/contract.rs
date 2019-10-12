use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;
use num_bigint::BigUint;
use num_traits::identities::Zero;
use num_traits::ops::checked::CheckedSub;

use protocol::traits::executor::RcCallContext;
use protocol::types::{AssetID, UserAddress};
use protocol::ProtocolResult;

use crate::adapter::DexAdapter;
use crate::error::DexError;
use crate::types::{
    CancelOrderArgs, Config, Deal, GetBalanceArgs, GetOrderbookArgs, GetPendingOrdersArgs, Order,
    OrderBook, OrderSide, OrderState, PlaceOrderArgs, SerUserBalance, TradingPair, UserBalance,
    WithdrawArgs,
};

pub struct DexContract<Adapter: DexAdapter> {
    adapter: Rc<RefCell<Adapter>>,
}

impl<Adapter> DexContract<Adapter>
where
    Adapter: DexAdapter,
{
    pub fn new(adapter: Rc<RefCell<Adapter>>) -> Self {
        Self { adapter }
    }

    pub fn call(&mut self, ictx: RcCallContext) -> ProtocolResult<Bytes> {
        let ctx = ictx.borrow();
        // dbg!(&ctx);
        let user = &ctx.origin;
        let method = ctx.method.clone();
        let args = ctx.args.clone();
        let _carrying_asset_option = &ctx.carrying_asset;

        let res = match method.as_str() {
            "update_fee_account" => serde_json::to_string(
                &self.update_fee_account(
                    serde_json::from_slice(&args[0])
                        .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?,
                )?,
            ),
            "new_config" => serde_json::to_string(
                &self.new_config(
                    serde_json::from_slice(&args[0])
                        .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?,
                )?,
            ),
            "add_trading_pair" => serde_json::to_string(
                &self.add_trading_pair(
                    serde_json::from_slice(&args[0])
                        .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?,
                )?,
            ),
            "deposit" => {
                // let carrying_asset = carrying_asset_option
                //     .clone()
                //     .ok_or_else(|| DexError::Contract("no carrying asset".to_owned()))?;
                // let res = &self.deposit(
                //     user.clone(),
                //     carrying_asset.asset_id,
                //     &carrying_asset.amount,
                // )?;
                let args: WithdrawArgs = serde_json::from_slice(&args[0])
                    .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?;
                let res = &self.deposit(user.clone(), args.asset_id, &args.amount)?;
                serde_json::to_string(&res)
            }
            "withdraw" => {
                let args: WithdrawArgs = serde_json::from_slice(&args[0])
                    .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?;
                let res = &self.withdraw(user.clone(), args.asset_id, &args.amount)?;
                serde_json::to_string(res)
            }
            "place_order" => {
                let args: PlaceOrderArgs = serde_json::from_slice(&args[0])
                    .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?;
                let res = &self.place_order(&user, args)?;
                serde_json::to_string(res)
            }
            "cancel_order" => {
                let args: CancelOrderArgs = serde_json::from_slice(&args[0])
                    .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?;
                let res = &self.cancel_order(user.clone(), &args.order_id)?;
                serde_json::to_string(res)
            }
            "clear" => serde_json::to_string(&self.clear()?),
            "get_trading_pairs" => {
                let res = self.get_trading_pairs()?;
                serde_json::to_string(&res)
            }
            "get_balance" => {
                let args: GetBalanceArgs = serde_json::from_slice(&args[0])
                    .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?;
                let user_balance = self.get_balance(&args.user)?;
                let res = SerUserBalance::from(user_balance);
                serde_json::to_string(&res)
            }
            "get_orderbook" => {
                let args: GetOrderbookArgs = serde_json::from_slice(&args[0])
                    .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?;
                let res = &self.get_orderbook(args.version, args.trading_pair_id)?;
                serde_json::to_string(res)
            }
            "get_pending_orders" => {
                let args: GetPendingOrdersArgs = serde_json::from_slice(&args[0])
                    .map_err(|_| DexError::ArgsError("args invalid".to_owned()))?;
                let res =
                    &self.get_pending_orders(args.version, args.trading_pair_id, &args.user)?;
                serde_json::to_string(res)
            }
            _ => return Err(DexError::Contract("unknown method".to_owned()).into()),
        }
        .unwrap();
        Ok(Bytes::from(res.as_bytes()))
    }

    pub fn get_trading_pairs(&self) -> ProtocolResult<Vec<TradingPair>> {
        self.adapter.borrow().get_trading_pairs()
    }

    pub fn get_balance(&self, user: &UserAddress) -> ProtocolResult<UserBalance> {
        self.adapter.borrow().get_balance(user)
    }

    pub fn get_orderbook(&self, version: u64, trading_pair_id: u64) -> ProtocolResult<OrderBook> {
        self.adapter
            .borrow()
            .get_orderbook(version, trading_pair_id)
    }

    pub fn get_pending_orders(
        &self,
        version: u64,
        trading_pair_id: u64,
        user: &UserAddress,
    ) -> ProtocolResult<Vec<Order>> {
        self.adapter
            .borrow()
            .get_pending_orders(version, trading_pair_id, user)
    }

    pub fn update_fee_account(&mut self, new_account: UserAddress) -> ProtocolResult<()> {
        self.adapter.borrow_mut().update_fee_account(new_account)
    }

    pub fn new_config(&mut self, config: Config) -> ProtocolResult<u64> {
        self.adapter.borrow_mut().new_config(config)
    }

    pub fn add_trading_pair(&mut self, pair: TradingPair) -> ProtocolResult<u64> {
        self.adapter.borrow_mut().add_trading_pair(pair)
    }

    pub fn deposit(
        &mut self,
        user: UserAddress,
        asset_id: AssetID,
        amount: &BigUint,
    ) -> ProtocolResult<()> {
        // TODO: handler transfer
        let mut user_balance = self.adapter.borrow().get_balance(&user)?;
        *user_balance
            .available
            .entry(asset_id)
            .or_insert_with(BigUint::zero) += amount;
        self.adapter.borrow_mut().set_balance(user, user_balance)
    }

    pub fn withdraw(
        &mut self,
        user: UserAddress,
        asset_id: AssetID,
        amount: &BigUint,
    ) -> ProtocolResult<()> {
        let mut user_balance = self.adapter.borrow().get_balance(&user)?;

        let balance = user_balance
            .available
            .entry(asset_id)
            .or_insert_with(BigUint::zero);
        *balance = balance
            .checked_sub(amount)
            .ok_or_else(|| DexError::Contract("not enough balance".to_owned()))?;
        // TODO: handler transfer
        self.adapter.borrow_mut().set_balance(user, user_balance)
    }

    pub fn place_order(
        &mut self,
        user: &UserAddress,
        args: PlaceOrderArgs,
    ) -> ProtocolResult<String> {
        let order_id = format!("{}-{}", user.as_hex(), args.nonce);
        if self.adapter.borrow_mut().get_order(&order_id)?.is_some() {
            return Err(
                DexError::Contract("order nonce exists for current user".to_owned()).into(),
            );
        }
        let order = Order {
            id:              order_id.clone(),
            nonce:           args.nonce.clone(),
            trading_pair_id: args.trading_pair_id,
            order_side:      args.order_side.clone(),
            price:           args.price.clone(),
            amount:          args.amount.clone(),
            version:         args.version,

            user:            user.clone(),
            unfilled_amount: args.amount.clone(),
            state:           OrderState::Pending,
        };

        let configs = self.adapter.borrow().get_configs()?;
        if args.version as usize > configs.len() {
            return Err(DexError::Contract("invalid version".to_owned()).into());
        }
        let trading_pairs = self.adapter.borrow().get_trading_pairs()?;
        if args.trading_pair_id as usize > trading_pairs.len() {
            return Err(DexError::Contract("invalid trading_pair_id".to_owned()).into());
        }
        let pair = &trading_pairs[args.trading_pair_id as usize];
        let (asset_id, amount) = match args.order_side {
            OrderSide::Buy => (&pair.quote_asset, &args.amount * &args.price * 100u64),
            OrderSide::Sell => (&pair.base_asset, &args.amount * 10_000_000_000u64),
        };

        let mut user_balance = self.adapter.borrow().get_balance(&user)?;
        let available_balance = user_balance
            .available
            .entry(asset_id.clone())
            .or_insert_with(BigUint::zero);
        *available_balance = available_balance
            .checked_sub(&amount)
            .ok_or_else(|| DexError::Contract("not enough balance".to_owned()))?;
        *user_balance
            .locked
            .entry(asset_id.clone())
            .or_insert_with(BigUint::zero) += amount;
        self.adapter
            .borrow_mut()
            .set_balance(user.clone(), user_balance)?;
        self.adapter.borrow_mut().set_order(order.clone())?;
        Ok(order_id)
    }

    pub fn cancel_order(&mut self, user: UserAddress, order_id: &str) -> ProtocolResult<()> {
        let order = self
            .adapter
            .borrow()
            .get_order(order_id)?
            .ok_or_else(|| DexError::Contract("order not exist".to_owned()))?;
        if order.state != OrderState::Pending {
            return Err(DexError::Contract("can only cancel pending order".to_owned()).into());
        }
        let trading_pairs = self.adapter.borrow().get_trading_pairs()?;
        let pair = &trading_pairs[order.trading_pair_id as usize];
        let (asset_id, amount) = match order.order_side {
            OrderSide::Buy => (
                &pair.quote_asset,
                &order.unfilled_amount * &order.price * 100u64,
            ),
            OrderSide::Sell => (&pair.base_asset, &order.unfilled_amount * 10_000_000_000u64),
        };

        let mut user_balance = self.adapter.borrow().get_balance(&user)?;
        *user_balance
            .available
            .entry(asset_id.clone())
            .or_insert_with(BigUint::zero) += &amount;
        *user_balance
            .locked
            .entry(asset_id.clone())
            .or_insert_with(BigUint::zero) -= &amount;

        self.adapter
            .borrow_mut()
            .set_balance(user.clone(), user_balance)?;
        self.adapter.borrow_mut().set_order(order.clone())?;
        Ok(())
    }

    fn clear_one_orderbook(&mut self, order_book: &mut OrderBook) -> ProtocolResult<Vec<Deal>> {
        let mut deals = vec![];
        let mut amount: BigUint;
        let mut price = BigUint::default();
        let mut sell_index = 0;
        let mut buy_index = 0;
        let sell_orders_len = order_book.sell_orders.len();
        let buy_orders_len = order_book.buy_orders.len();
        while sell_index < sell_orders_len
            && buy_index < buy_orders_len
            && order_book.sell_orders[sell_index].price <= order_book.buy_orders[buy_index].price
        {
            price = order_book.sell_orders[sell_index].price.clone();
            let buy_order_id = order_book.buy_orders[buy_index].id.clone();
            let sell_order_id = order_book.sell_orders[sell_index].id.clone();
            if order_book.buy_orders[buy_index].amount < order_book.sell_orders[sell_index].amount {
                amount = order_book.buy_orders[buy_index].amount.clone();
                buy_index += 1;
                order_book.sell_orders[sell_index].amount -= &amount;
            } else if order_book.buy_orders[buy_index].amount
                > order_book.sell_orders[sell_index].amount
            {
                amount = order_book.sell_orders[sell_index].amount.clone();
                sell_index += 1;
                order_book.buy_orders[buy_index].amount -= &amount;
            } else {
                amount = order_book.buy_orders[buy_index].amount.clone();
                buy_index += 1;
                sell_index += 1;
            }
            deals.push(Deal {
                price: price.clone(),
                amount: amount.clone(),
                buy_order_id,
                sell_order_id,
            });
        }
        for deal in &mut deals {
            deal.price = price.clone();
        }
        Ok(deals)
    }

    pub fn clear(&mut self) -> ProtocolResult<Vec<Deal>> {
        let mut deals = vec![];
        let version_num = self.adapter.borrow().get_configs()?.len() as u64;
        let trading_pair_num = self.adapter.borrow().get_trading_pairs()?.len() as u64;
        for version in 0..version_num {
            for trading_pair_id in 0..trading_pair_num {
                let mut order_book = self
                    .adapter
                    .borrow()
                    .get_orderbook(version, trading_pair_id)?;
                for deal in self.clear_one_orderbook(&mut order_book)? {
                    self.trade(&deal)?;
                    deals.push(deal);
                }
            }
        }
        Ok(deals)
    }

    fn require(&self, condition: bool, msg: &str) -> ProtocolResult<()> {
        if condition {
            Ok(())
        } else {
            Err(DexError::Contract(msg.to_owned()).into())
        }
    }

    fn trade(&mut self, deal: &Deal) -> ProtocolResult<()> {
        let configs = self.adapter.borrow().get_configs()?;
        let trading_pairs = self.adapter.borrow().get_trading_pairs()?;
        let mut buy_order: Order = self
            .adapter
            .borrow()
            .get_order(&deal.buy_order_id)?
            .unwrap()
            .clone();
        let mut sell_order: Order = self
            .adapter
            .borrow()
            .get_order(&deal.sell_order_id)?
            .unwrap()
            .clone();

        self.require(
            buy_order.state == OrderState::Pending,
            "buy order is not active",
        )?;
        self.require(
            sell_order.state == OrderState::Pending,
            "sell order is not active",
        )?;
        self.require(
            buy_order.trading_pair_id == sell_order.trading_pair_id,
            "trading pair not match",
        )?;
        self.require(buy_order.version == sell_order.version, "version not match")?;
        self.require(
            deal.price <= buy_order.price,
            "deal price should be less or equal to buy price",
        )?;
        self.require(
            deal.price >= sell_order.price,
            "deal price should be greater or equal to sell price",
        )?;

        buy_order.unfilled_amount = buy_order
            .unfilled_amount
            .checked_sub(&deal.amount)
            .ok_or_else(|| DexError::Contract("buy order amount not enough".to_owned()))?;
        sell_order.unfilled_amount = sell_order
            .unfilled_amount
            .checked_sub(&deal.amount)
            .ok_or_else(|| DexError::Contract("sell order amount not enough".to_owned()))?;
        if buy_order.unfilled_amount.is_zero() {
            buy_order.state = OrderState::FullFilled;
        }
        if sell_order.unfilled_amount.is_zero() {
            sell_order.state = OrderState::FullFilled;
        }

        let pair = &trading_pairs[buy_order.trading_pair_id as usize];
        let config = &configs[buy_order.version as usize];
        let fee_rate = config.fee_rate;
        let base_asset = &pair.base_asset;
        let quote_asset = &pair.quote_asset;
        // the unit of amount and price is 10^-8, the unit of asset amount is 10^18
        // (amount / 10^8) * (price / 10^8) * 10^18
        let quote_asset_amount = &deal.amount * &deal.price * 100u64;
        // amount / 10^8 * 10^18
        let base_asset_amount = &deal.amount * 1e10 as u64;

        let quote_asset_fee = &quote_asset_amount * fee_rate / 1e8 as u64;
        let base_asset_fee = &base_asset_amount * fee_rate / 1e8 as u64;

        let buy_unlock_quote_amount = &deal.amount * &buy_order.price * 100u64;
        let sell_unlock_base_amount = &deal.amount * 1e10 as u64;

        let mut buy_user_balance: UserBalance =
            self.adapter.borrow().get_balance(&buy_order.user)?;
        let mut sell_user_balance: UserBalance =
            self.adapter.borrow().get_balance(&sell_order.user)?;
        let fee_account = self.adapter.borrow().get_fee_account()?;
        let mut fee_user_balance: UserBalance = self.adapter.borrow().get_balance(&fee_account)?;

        // unlock
        *buy_user_balance
            .locked
            .entry(quote_asset.clone())
            .or_insert_with(BigUint::zero) -= &buy_unlock_quote_amount;
        *sell_user_balance
            .locked
            .entry(base_asset.clone())
            .or_insert_with(BigUint::zero) -= &sell_unlock_base_amount;

        // exchange
        *buy_user_balance
            .available
            .entry(quote_asset.clone())
            .or_insert_with(BigUint::zero) += &buy_unlock_quote_amount - &quote_asset_amount;
        *buy_user_balance
            .available
            .entry(base_asset.clone())
            .or_insert_with(BigUint::zero) += &base_asset_amount - &base_asset_fee;
        *sell_user_balance
            .available
            .entry(base_asset.clone())
            .or_insert_with(BigUint::zero) += &sell_unlock_base_amount - &base_asset_amount;
        *sell_user_balance
            .available
            .entry(quote_asset.clone())
            .or_insert_with(BigUint::zero) += &quote_asset_amount - &quote_asset_fee;
        // fee
        *fee_user_balance
            .available
            .entry(base_asset.clone())
            .or_insert_with(BigUint::zero) += &base_asset_fee;
        *fee_user_balance
            .available
            .entry(quote_asset.clone())
            .or_insert_with(BigUint::zero) += &quote_asset_fee;

        // set all data
        self.adapter
            .borrow_mut()
            .set_balance(fee_account.clone(), fee_user_balance)?;
        self.adapter
            .borrow_mut()
            .set_balance(buy_order.user.clone(), buy_user_balance)?;
        self.adapter
            .borrow_mut()
            .set_balance(sell_order.user.clone(), sell_user_balance)?;
        self.adapter.borrow_mut().set_order(buy_order)?;
        self.adapter.borrow_mut().set_order(sell_order)?;

        Ok(())
    }
}
