use serde::Serialize;

use core_types::{self, Hash};

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct TxResponse {
    pub hash: Hash,
    pub status: String,
}

impl TxResponse {
    pub fn new(hash: Hash, status: String) -> Self {
        TxResponse { hash, status }
    }
}

/// Struct used and only used for CITA jsonrpc.
pub mod cita {
    use core_serialization::generate_module_for;
    use core_types::Address;
    generate_module_for!([blockchain]);

    // #[derive(Clone, PartialEq, ::prost::Message)]
    // pub struct Transaction {
    //     #[prost(string, tag="1")]
    //     pub to: std::string::String,
    //     #[prost(string, tag="2")]
    //     pub nonce: std::string::String,
    //     #[prost(uint64, tag="3")]
    //     pub quota: u64,
    //     #[prost(uint64, tag="4")]
    //     pub valid_until_block: u64,
    //     #[prost(bytes, tag="5")]
    //     pub data: std::vec::Vec<u8>,
    //     #[prost(bytes, tag="6")]
    //     pub value: std::vec::Vec<u8>,
    //     #[prost(uint32, tag="7")]
    //     pub chain_id: u32,
    //     #[prost(uint32, tag="8")]
    //     pub version: u32,
    //     #[prost(bytes, tag="9")]
    //     pub to_v1: std::vec::Vec<u8>,
    //     #[prost(bytes, tag="10")]
    //     pub chain_id_v1: std::vec::Vec<u8>,
    // }

    // #[derive(Clone, PartialEq, ::prost::Message)]
    // pub struct UnverifiedTransaction {
    //     #[prost(message, optional, tag="1")]
    //     pub transaction: ::std::option::Option<Transaction>,
    //     #[prost(bytes, tag="2")]
    //     pub signature: std::vec::Vec<u8>,
    //     #[prost(enumeration="Crypto", tag="3")]
    //     pub crypto: i32,
    // }

    // #[derive(Clone, PartialEq, ::prost::Message)]
    // pub struct SignedTransaction {
    //     #[prost(message, optional, tag="1")]
    //     pub transaction_with_sig: ::std::option::Option<UnverifiedTransaction>,
    //     #[prost(bytes, tag="2")]
    //     pub tx_hash: std::vec::Vec<u8>,
    //     #[prost(bytes, tag="3")]
    //     pub signer: std::vec::Vec<u8>,
    // }

    impl Into<core_types::Transaction> for Transaction {
        fn into(self) -> core_types::Transaction {
            let to = match self.version {
                1 => {
                    if self.to_v1.is_empty() {
                        None
                    } else {
                        Some(Address::from_bytes(&self.to_v1).unwrap()) // TODO: Use try_into instead
                    }
                }
                _ => {
                    if self.to.is_empty() {
                        None
                    } else {
                        Some(Address::from_hex(&self.to).unwrap()) // TODO: Use try_into instead
                    }
                }
            };
            let chain_id = match self.version {
                1 => self.chain_id_v1,
                _ => self.chain_id.to_be_bytes().to_vec(),
            };
            core_types::Transaction {
                to,
                nonce: self.nonce,
                quota: self.quota,
                valid_until_block: self.valid_until_block,
                data: self.data,
                value: self.value,
                chain_id,
            }
        }
    }

    impl Into<core_types::UnverifiedTransaction> for UnverifiedTransaction {
        fn into(self) -> core_types::UnverifiedTransaction {
            core_types::UnverifiedTransaction {
                transaction: self.transaction.unwrap_or_default().into(),
                signature: self.signature,
            }
        }
    }
}
