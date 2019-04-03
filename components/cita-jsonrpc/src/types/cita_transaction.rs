#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Transaction {
    #[prost(string, tag="1")]
    pub to: std::string::String,
    #[prost(string, tag="2")]
    pub nonce: std::string::String,
    #[prost(uint64, tag="3")]
    pub quota: u64,
    #[prost(uint64, tag="4")]
    pub valid_until_block: u64,
    #[prost(bytes, tag="5")]
    pub data: std::vec::Vec<u8>,
    #[prost(bytes, tag="6")]
    pub value: std::vec::Vec<u8>,
    #[prost(uint32, tag="7")]
    pub chain_id: u32,
    #[prost(uint32, tag="8")]
    pub version: u32,
    #[prost(bytes, tag="9")]
    pub to_v1: std::vec::Vec<u8>,
    #[prost(bytes, tag="10")]
    pub chain_id_v1: std::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UnverifiedTransaction {
    #[prost(message, optional, tag="1")]
    pub transaction: ::std::option::Option<Transaction>,
    #[prost(bytes, tag="2")]
    pub signature: std::vec::Vec<u8>,
    #[prost(enumeration="Crypto", tag="3")]
    pub crypto: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Crypto {
    Secp = 0,
    Sm2 = 1,
}
