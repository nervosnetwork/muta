use std::collections::BTreeMap;
use std::convert::From;
use std::str::FromStr;

use bytes::Bytes;
use num_bigint::BigUint;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use protocol::traits::executor::{ContractSchema, ContractSer};
use protocol::types::{AssetID, UserAddress};
use protocol::ProtocolResult;

use crate::error::DexError;

pub const FEE_ACCOUNT_KEY: &str = "fee_account";
pub const CONFIGS_KEY: &str = "configs";
pub const TRADING_PAIRS_KEY: &str = "trading_pairs";
pub const ADMINS_KEY: &str = "admins";
pub const BALANCES_KEY_PREFIX: &str = "b";
pub const ORDERS_KEY_PREFIX: &str = "o";

#[derive(Debug, Clone, PartialEq)]
pub struct Ser<T>(pub T);

impl Serialize for Ser<BigUint> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex = format!("{}", self.0);
        serializer.serialize_str(&hex)
    }
}

impl<'de> Deserialize<'de> for Ser<BigUint> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let biguint = BigUint::from_str(&s).map_err(de::Error::custom)?;
        Ok(Ser(biguint))
    }
}

impl From<BigUint> for Ser<BigUint> {
    fn from(item: BigUint) -> Self {
        Ser(item)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Config {
    pub fee_rate: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradingPair {
    pub symbol:      String,
    pub base_asset:  AssetID,
    pub quote_asset: AssetID,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderState {
    Pending,
    Canceled,
    FullFilled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub nonce: String,
    pub trading_pair_id: u64,
    pub order_side: OrderSide,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub price: BigUint,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub amount: BigUint,
    pub version: u64,
    pub user: UserAddress,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub unfilled_amount: BigUint,
    pub state: OrderState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deal {
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub price: BigUint,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub amount: BigUint,
    pub buy_order_id: String,
    pub sell_order_id: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UserBalance {
    pub available: BTreeMap<AssetID, BigUint>,
    pub locked:    BTreeMap<AssetID, BigUint>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerUserBalance {
    pub available: BTreeMap<AssetID, Ser<BigUint>>,
    pub locked:    BTreeMap<AssetID, Ser<BigUint>>,
}

impl From<UserBalance> for SerUserBalance {
    fn from(item: UserBalance) -> Self {
        let mut res: SerUserBalance = Default::default();
        for (asset_id, amount) in item.available.into_iter() {
            res.available.insert(asset_id, Ser(amount));
        }
        for (asset_id, amount) in item.locked.into_iter() {
            res.locked.insert(asset_id, Ser(amount));
        }
        res
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositArgs {
    user: UserAddress,
    asset_id: AssetID,
    #[serde(with = "serde_with::rust::display_fromstr")]
    amount: BigUint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawArgs {
    pub asset_id: AssetID,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub amount: BigUint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceOrderArgs {
    pub nonce: String,
    pub trading_pair_id: u64,
    pub order_side: OrderSide,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub price: BigUint,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub amount: BigUint,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderArgs {
    pub order_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBalanceArgs {
    pub user: UserAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOrderbookArgs {
    pub version:         u64,
    pub trading_pair_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPendingOrdersArgs {
    pub version:         u64,
    pub trading_pair_id: u64,
    pub user:            UserAddress,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OrderBook {
    pub trading_pair_id: u64,
    pub version:         u64,
    pub buy_orders:      Vec<Order>, // sorted by price, from highest to lowest
    pub sell_orders:     Vec<Order>, // sorted by price, from lowest to highest
}

#[derive(Debug, Clone, Default)]
pub struct PendingOrders {
    pub inner: BTreeMap<String, Order>,
}

impl rlp::Encodable for PendingOrders {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(self.inner.len());
        for (_, order) in self.inner.iter() {
            s.append(order);
        }
    }
}

impl rlp::Decodable for PendingOrders {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let mut res = Self::default();
        for rr in r.iter() {
            let order: Order = rr.as_val()?;
            res.inner.insert(order.id.clone(), order);
        }
        Ok(res)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ConfigsValue(pub Vec<Config>);

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TradingPairsValue(pub Vec<TradingPair>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserAddresssValue(pub Vec<UserAddress>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrdersValue(pub Order);

impl rlp::Encodable for Config {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.append(&self.fee_rate);
    }
}

impl rlp::Decodable for Config {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Self {
            fee_rate: r.as_val()?,
        })
    }
}

impl rlp::Encodable for TradingPair {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3);
        s.append(&self.symbol);
        s.append(&self.base_asset.as_bytes().to_vec());
        s.append(&self.quote_asset.as_bytes().to_vec());
    }
}

impl rlp::Decodable for TradingPair {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Self {
            symbol:      r.at(0)?.as_val()?,
            base_asset:  AssetID::from_bytes(Bytes::from(r.at(1)?.data()?))
                .map_err(|_| rlp::DecoderError::Custom("base_asset invalid"))?,
            quote_asset: AssetID::from_bytes(Bytes::from(r.at(2)?.data()?))
                .map_err(|_| rlp::DecoderError::Custom("quote_asset invalid"))?,
        })
    }
}

impl rlp::Encodable for UserBalance {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2);
        s.begin_list(self.available.len());
        for (asset_id, amount) in self.available.iter() {
            s.begin_list(2);
            s.append(&asset_id.as_bytes().to_vec());
            s.append(&amount.to_bytes_be());
        }
        s.begin_list(self.locked.len());
        for (asset_id, amount) in self.locked.iter() {
            s.begin_list(2);
            s.append(&asset_id.as_bytes().to_vec());
            s.append(&amount.to_bytes_be());
        }
    }
}

fn bytes_to_assetid(bytes: &[u8], annotation: &'static str) -> Result<AssetID, rlp::DecoderError> {
    AssetID::from_bytes(Bytes::from(bytes)).map_err(|_| rlp::DecoderError::Custom(annotation))
}

fn bytes_to_user_address(
    bytes: &[u8],
    annotation: &'static str,
) -> Result<UserAddress, rlp::DecoderError> {
    UserAddress::from_bytes(Bytes::from(bytes)).map_err(|_| rlp::DecoderError::Custom(annotation))
}

impl rlp::Decodable for UserBalance {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let mut user_balance = UserBalance::default();
        for available in r.at(0)?.iter() {
            let asset_id =
                bytes_to_assetid(available.at(0)?.data()?, "available asset_id invalid")?;
            let amount = BigUint::from_bytes_be(available.at(1)?.data()?);
            user_balance.available.insert(asset_id, amount);
        }
        for locked in r.at(1)?.iter() {
            let asset_id = bytes_to_assetid(locked.at(0)?.data()?, "locked asset_id invalid")?;
            let amount = BigUint::from_bytes_be(locked.at(1)?.data()?);
            user_balance.locked.insert(asset_id, amount);
        }
        Ok(user_balance)
    }
}

impl rlp::Encodable for Order {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(10);
        s.append(&self.id);
        s.append(&self.nonce);
        s.append(&self.trading_pair_id);
        s.append(&match self.order_side {
            OrderSide::Buy => 1u8,
            OrderSide::Sell => 2u8,
        });
        s.append(&self.price.to_bytes_be());
        s.append(&self.amount.to_bytes_be());
        s.append(&self.version);
        s.append(&self.user.as_bytes().to_vec());
        s.append(&self.unfilled_amount.to_bytes_be());
        s.append(&match self.state {
            OrderState::Pending => 1u8,
            OrderState::Canceled => 2u8,
            OrderState::FullFilled => 3u8,
        });
    }
}

impl rlp::Decodable for Order {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Self {
            id:              r.at(0)?.as_val()?,
            nonce:           r.at(1)?.as_val()?,
            trading_pair_id: r.at(2)?.as_val()?,
            order_side:      match r.at(3)?.as_val()? {
                1u8 => OrderSide::Buy,
                2u8 => OrderSide::Sell,
                _ => return Err(rlp::DecoderError::Custom("order order_side invalid")),
            },
            price:           BigUint::from_bytes_be(r.at(4)?.data()?),
            amount:          BigUint::from_bytes_be(r.at(5)?.data()?),
            version:         r.at(6)?.as_val()?,
            user:            bytes_to_user_address(r.at(7)?.data()?, "order user invalid")?,
            unfilled_amount: BigUint::from_bytes_be(r.at(8)?.data()?),
            state:           match r.at(9)?.as_val()? {
                1u8 => OrderState::Pending,
                2u8 => OrderState::Canceled,
                3u8 => OrderState::FullFilled,
                _ => return Err(rlp::DecoderError::Custom("order order_state invalid")),
            },
        })
    }
}

impl rlp::Encodable for ConfigsValue {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.append_list(&self.0);
    }
}

impl rlp::Decodable for ConfigsValue {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Self(r.as_list()?))
    }
}

impl rlp::Encodable for TradingPairsValue {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.append_list(&self.0);
    }
}

impl rlp::Decodable for TradingPairsValue {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Self(r.as_list()?))
    }
}

pub struct FixedBalanceSchema;
impl ContractSchema for FixedBalanceSchema {
    type Key = FixedBalanceKey;
    type Value = FixedBalanceValue;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FixedBalanceKey(pub UserAddress);

#[derive(Clone, Debug)]
pub struct FixedBalanceValue(pub UserBalance);

impl ContractSer for FixedBalanceKey {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(self.0.as_bytes())
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(Self(UserAddress::from_bytes(bytes)?))
    }
}

impl ContractSer for FixedBalanceValue {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(Self(
            rlp::decode(&bytes).map_err(DexError::FixedTypesError)?,
        ))
    }
}

pub struct FixedOrderSchema;
impl ContractSchema for FixedOrderSchema {
    type Key = FixedOrderKey;
    type Value = FixedOrderValue;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FixedOrderKey(pub String);

#[derive(Clone, Debug)]
pub struct FixedOrderValue(pub Order);

impl ContractSer for FixedOrderKey {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(self.0.as_bytes()))
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(Self(String::from_utf8_lossy(&bytes).to_string()))
    }
}

impl ContractSer for FixedOrderValue {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(Self(
            rlp::decode(&bytes).map_err(DexError::FixedTypesError)?,
        ))
    }
}

pub struct FixedPendingOrderSchema;
impl ContractSchema for FixedPendingOrderSchema {
    type Key = FixedPendingOrderKey;
    type Value = PendingOrders;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FixedPendingOrderKey {
    pub version:         u64,
    pub trading_pair_id: u64,
}

impl ContractSer for FixedPendingOrderKey {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(
            [
                self.version.to_be_bytes(),
                self.trading_pair_id.to_be_bytes(),
            ]
            .concat(),
        ))
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        let mut version_bytes = [0u8; 8];
        version_bytes.copy_from_slice(&bytes.slice(0, 9)[..]);
        let mut trading_pair_id_bytes = [0u8; 8];
        trading_pair_id_bytes.copy_from_slice(&bytes.slice(9, 17));
        Ok(Self {
            version:         u64::from_be_bytes(version_bytes),
            trading_pair_id: u64::from_be_bytes(trading_pair_id_bytes),
        })
    }
}

impl ContractSer for PendingOrders {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(&bytes).map_err(DexError::FixedTypesError)?)
    }
}
