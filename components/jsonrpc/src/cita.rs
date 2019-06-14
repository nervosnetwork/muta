/// Struct used and only used for CITA jsonrpc.
/// Note: Most of these codes copies from "https://github.com/cryptape/cita-common"
use std::collections::HashMap;

use ethbloom::Bloom;
use hex;
use numext_fixed_hash::H160;
use numext_fixed_uint::U256;
use rlp::{Encodable, RlpStream};
use serde::de::{self, DeserializeOwned, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{self, Value};

pub use common_cita::{Transaction, UnverifiedTransaction};
use core_types::{self, Address, Hash};

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct TxResponse {
    pub hash:   Hash,
    pub status: String,
}

impl TxResponse {
    pub fn new(hash: Hash, status: String) -> Self {
        TxResponse { hash, status }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum Proof {
    Raft,
    Bft(BftProof),
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct BftProof {
    pub proposal: Hash,
    pub height:   usize,
    pub round:    usize,
    pub commits:  HashMap<Address, String>,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct FullTransaction {
    pub hash:    Hash,
    pub content: Data,
    pub from:    Address,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct RpcTransaction {
    pub hash: Hash,
    pub content: Data,
    pub from: Address,
    #[serde(rename = "blockNumber")]
    pub block_number: Uint,
    #[serde(rename = "blockHash")]
    pub block_hash: Hash,
    pub index: Uint,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(untagged)]
pub enum BlockTransaction {
    Full(FullTransaction),
    Hash(Hash),
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct BlockBody {
    pub transactions: Vec<BlockTransaction>,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct BlockHeader {
    pub timestamp: u64,
    #[serde(rename = "prevHash")]
    pub prev_hash: Hash,
    pub number: Uint,
    #[serde(rename = "stateRoot")]
    pub state_root: Hash,
    #[serde(rename = "transactionsRoot")]
    pub transactions_root: Hash,
    #[serde(rename = "receiptsRoot")]
    pub receipts_root: Hash,
    #[serde(rename = "quotaUsed")]
    pub quota_used: Uint,
    pub proof: Option<Proof>,
    pub proposer: Address,
}

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    pub version: u32,
    pub hash:    Hash,
    pub header:  BlockHeader,
    pub body:    BlockBody,
}

#[derive(Debug, Serialize, Hash, Clone)]
pub struct Log {
    pub address: Address,
    pub topics: Vec<Hash>,
    pub data: Data,
    #[serde(rename = "blockHash")]
    pub block_hash: Option<Hash>,
    #[serde(rename = "blockNumber")]
    pub block_number: Option<Uint>,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<Hash>,
    #[serde(rename = "transactionIndex")]
    pub transaction_index: Option<Uint>,
    #[serde(rename = "logIndex")]
    pub log_index: Option<Uint>,
    #[serde(rename = "transactionLogIndex")]
    pub transaction_log_index: Option<Uint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Receipt {
    /// Transaction Hash
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<Hash>,
    /// Transaction index
    #[serde(rename = "transactionIndex")]
    pub transaction_index: Option<Uint>,
    /// Block hash
    #[serde(rename = "blockHash")]
    pub block_hash: Option<Hash>,
    /// Block
    #[serde(rename = "blockNumber")]
    pub block_number: Option<Uint>,
    /// Cumulative quota used
    #[serde(rename = "cumulativeQuotaUsed")]
    pub cumulative_quota_used: Uint,
    /// Quota used
    #[serde(rename = "quotaUsed")]
    pub quota_used: Option<Uint>,
    /// Contract address
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<Address>,
    /// Logs
    pub logs: Vec<Log>,
    /// State Root
    #[serde(rename = "root")]
    pub state_root: Option<Hash>,
    /// Logs bloom
    #[serde(rename = "logsBloom")]
    pub logs_bloom: Bloom,
    /// Receipt error message
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Default, Hash, Clone)]
pub struct Data(Vec<u8>);

impl Data {
    pub fn new(data: Vec<u8>) -> Data {
        Data(data)
    }
}

impl Encodable for Data {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.0);
    }
}

impl Serialize for Data {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(self.0.as_slice())))
    }
}

impl<'de> Deserialize<'de> for Data {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(DataVisitor)
    }
}

struct DataVisitor;

impl<'de> Visitor<'de> for DataVisitor {
    type Value = Data;

    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        formatter.write_str("Data")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.is_empty() {
            Ok(Data::new(Vec::new()))
        } else if value.len() >= 2
            && (&value[0..2] == "0x" || &value[0..2] == "0X")
            && value.len() & 1 == 0
        {
            let data = hex::FromHex::from_hex(&value[2..]).map_err(|_| {
                if value.len() > 12 {
                    E::custom(format!(
                        "invalid hexadecimal string: [{}..(omit {})..{}]",
                        &value[..6],
                        value.len() - 12,
                        &value[value.len() - 6..value.len()]
                    ))
                } else {
                    E::custom(format!("invalid hexadecimal string: [{}]", value))
                }
            })?;
            Ok(Data::new(data))
        } else if value.len() > 12 {
            Err(E::custom(format!(
                "invalid format: [{}..(omit {})..{}]",
                &value[..6],
                value.len() - 12,
                &value[value.len() - 6..value.len()]
            )))
        } else {
            Err(E::custom(format!("invalid format: [{}]", value)))
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(value.as_ref())
    }
}

impl From<Vec<u8>> for Data {
    fn from(data: Vec<u8>) -> Data {
        Data::new(data)
    }
}

impl Into<Vec<u8>> for Data {
    fn into(self) -> Vec<u8> {
        self.0
    }
}

/// Fixed length bytes (wrapper structure around H256).
#[derive(Debug, PartialEq, Eq, Default, Hash, Clone)]
pub struct Data32(Hash);

/// Fixed length bytes (wrapper structure around H160).
#[derive(Debug, PartialEq, Eq, Default, Hash, Clone)]
pub struct Data20(H160);

struct Data32Visitor;
struct Data20Visitor;

impl Data32 {
    pub fn new(data: Hash) -> Self {
        Self(data)
    }
}

impl Serialize for Data32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0.as_bytes()))
    }
}

impl<'de> Deserialize<'de> for Data32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(Data32Visitor)
    }
}

impl<'de> Visitor<'de> for Data32Visitor {
    type Value = Data32;

    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        formatter.write_str(stringify!(Data32))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.len() == 2 + 32usize * 2 && (&value[0..2] == "0x" || &value[0..2] == "0X") {
            let data = Hash::from_hex(&value[2..]).map_err(|_| {
                if value.len() > 12 {
                    E::custom(format!(
                        "invalid hexadecimal string: [{}..(omit {})..{}]",
                        &value[..6],
                        value.len() - 12,
                        &value[value.len() - 6..value.len()]
                    ))
                } else {
                    E::custom(format!("invalid hexadecimal string: [{}]", value))
                }
            })?;
            Ok(Data32::new(data))
        } else {
            if value.len() > 12 {
                Err(E::custom(format!(
                    "invalid format: [{}..(omit {})..{}]",
                    &value[..6],
                    value.len() - 12,
                    &value[value.len() - 6..value.len()]
                )))
            } else {
                Err(E::custom(format!("invalid format: [{}]", value)))
            }
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(value.as_ref())
    }
}

impl From<Hash> for Data32 {
    fn from(data: Hash) -> Data32 {
        Data32::new(data)
    }
}

impl Into<Hash> for Data32 {
    fn into(self) -> Hash {
        self.0
    }
}

impl Into<Vec<u8>> for Data32 {
    fn into(self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }
}

impl Data20 {
    pub fn new(data: H160) -> Self {
        Self(data)
    }
}

impl Serialize for Data20 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0.as_bytes()))
    }
}

impl<'de> Deserialize<'de> for Data20 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(Data20Visitor)
    }
}

impl<'de> Visitor<'de> for Data20Visitor {
    type Value = Data20;

    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        formatter.write_str(stringify!(Data20))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.len() == 2 + 20usize * 2 && (&value[0..2] == "0x" || &value[0..2] == "0X") {
            let data = H160::from_hex_str(&value[2..]).map_err(|_| {
                if value.len() > 12 {
                    E::custom(format!(
                        "invalid hexadecimal string: [{}..(omit {})..{}]",
                        &value[..6],
                        value.len() - 12,
                        &value[value.len() - 6..value.len()]
                    ))
                } else {
                    E::custom(format!("invalid hexadecimal string: [{}]", value))
                }
            })?;
            Ok(Data20::new(data))
        } else {
            if value.len() > 12 {
                Err(E::custom(format!(
                    "invalid format: [{}..(omit {})..{}]",
                    &value[..6],
                    value.len() - 12,
                    &value[value.len() - 6..value.len()]
                )))
            } else {
                Err(E::custom(format!("invalid format: [{}]", value)))
            }
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(value.as_ref())
    }
}

impl From<H160> for Data20 {
    fn from(data: H160) -> Data20 {
        Data20::new(data)
    }
}

impl Into<H160> for Data20 {
    fn into(self) -> H160 {
        self.0
    }
}

impl Into<Vec<u8>> for Data20 {
    fn into(self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }
}

#[derive(Debug, Hash, Clone, Serialize)]
#[serde(untagged)]
pub enum VariadicValue<T: Serialize> {
    Null,
    Single(T),
    Multiple(Vec<T>),
}

impl<T> VariadicValue<T>
where
    T: Serialize,
{
    pub fn null() -> Self {
        VariadicValue::Null
    }

    pub fn single(data: T) -> Self {
        VariadicValue::Single(data)
    }

    pub fn multiple(data: Vec<T>) -> Self {
        VariadicValue::Multiple(data)
    }
}

impl<'de, T> Deserialize<'de> for VariadicValue<T>
where
    T: DeserializeOwned + Serialize,
{
    fn deserialize<D>(deserializer: D) -> Result<VariadicValue<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Value = Deserialize::deserialize(deserializer)?;

        if v.is_null() {
            return Ok(VariadicValue::Null);
        }

        serde_json::from_value(v.clone())
            .map(VariadicValue::Single)
            .or_else(|_| serde_json::from_value(v).map(VariadicValue::Multiple))
            .map_err(|_| de::Error::custom("invalid type"))
    }
}

pub type FilterAddress = VariadicValue<Data20>;
pub type Topic = VariadicValue<Data32>;

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    #[serde(rename = "fromBlock")]
    pub from_block: Option<String>,
    #[serde(rename = "toBlock")]
    pub to_block: Option<String>,
    pub address: Option<FilterAddress>,
    pub topics: Option<Vec<Topic>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

// Results of the filter_changes RPC.
#[derive(Debug, Clone)]
pub enum FilterChanges {
    /// New logs.
    Logs(Vec<Log>),
    /// New hashes (block or transactions)
    Hashes(Vec<Hash>),
}

impl Serialize for FilterChanges {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            FilterChanges::Logs(ref logs) => logs.serialize(s),
            FilterChanges::Hashes(ref hashes) => hashes.serialize(s),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct CallRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Data20>,
    pub to: Data20,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Data>,
}

impl CallRequest {
    pub fn new(from: Option<Data20>, to: Data20, data: Option<Data>) -> Self {
        CallRequest { from, to, data }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct MetaData {
    /// The id of current chain
    #[serde(rename = "chainId")]
    pub chain_id: u32,
    /// The id v1 of current chain
    #[serde(rename = "chainIdV1")]
    pub chain_id_v1: Uint,
    /// The name of current chain
    #[serde(rename = "chainName")]
    pub chain_name: String,
    /// The operator of current chain
    pub operator: String,
    /// Current operator's website URL
    pub website: String,
    /// Genesis block's timestamp (milliseconds)
    #[serde(rename = "genesisTimestamp")]
    pub genesis_timestamp: u64,
    /// Node address list which validate blocks
    pub validators: Vec<Data20>,
    /// The interval time for creating a block (milliseconds)
    #[serde(rename = "blockInterval")]
    pub block_interval: u64,
    /// Token name
    #[serde(rename = "tokenName")]
    pub token_name: String,
    #[serde(rename = "tokenSymbol")]
    pub token_symbol: String,
    #[serde(rename = "tokenAvatar")]
    pub token_avatar: String,
    pub version: u32,
    #[serde(rename = "economicalModel")]
    pub economical_model: u64,
}

#[derive(Debug, Clone)]
pub struct ProofNode<T> {
    pub is_right: bool,
    pub hash:     T,
}

/// Structure encodable to RLP
impl<T> Encodable for ProofNode<T>
where
    T: Default + Clone + Encodable,
{
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2);
        s.append(&self.is_right);
        s.append(&self.hash);
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Uint(U256);

impl Uint {
    pub fn as_hex(&self) -> String {
        format!("{:x}", self.0)
    }
}

impl From<u64> for Uint {
    fn from(u: u64) -> Self {
        Uint(U256::from(u))
    }
}

impl Serialize for Uint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", self.as_hex()))
    }
}

impl<'de> Deserialize<'de> for Uint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut bytes = [0u8; 32 * 8];
        let wrote = ethereum_types_serialize::deserialize_check_len(
            deserializer,
            ethereum_types_serialize::ExpectedLen::Between(0, &mut bytes),
        )?;

        Ok(U256::from_big_endian(&bytes[0..wrote]).map(Uint).unwrap())
    }
}

#[derive(Default, Debug, Clone)]
pub struct StateProof {
    pub address:       Address,
    pub account_proof: Vec<Data>,
    pub key:           Hash,
    pub value_proof:   Vec<Data>,
}

impl Encodable for StateProof {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.address);
        s.append_list(&self.account_proof);
        s.append(&self.key);
        s.append_list(&self.value_proof);
    }
}

// TxProof is not fully same as cita. muta's core types used to fill the struct,
// not cita.
#[derive(Debug, Clone)]
pub struct TxProof {
    pub tx:                   core_types::SignedTransaction,
    pub receipt:              core_types::Receipt,
    pub receipt_proof:        Vec<ProofNode<Hash>>,
    pub block_header:         core_types::BlockHeader,
    pub next_proposal_header: core_types::BlockHeader,
    pub proposal_proof:       core_types::Proof,
}

impl Encodable for TxProof {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(6);
        s.append(&self.tx);
        s.append(&self.receipt);
        s.append_list(&self.receipt_proof);
        s.append(&self.block_header);
        s.append(&self.next_proposal_header);
        s.append(&self.proposal_proof);
    }
}
