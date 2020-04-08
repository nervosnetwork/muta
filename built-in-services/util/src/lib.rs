#[cfg(test)]
mod tests;
pub mod types;


use std::collections::BTreeMap;

use bytes::Bytes;
use hasher::{Hasher, HasherKeccak};

use binding_macro::{cycles, genesis, service};
use common_crypto::{
    Crypto, HashValue, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey, Secp256k1PublicKey,
    Secp256k1Signature, Signature, ToPublicKey,
};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK, StoreMap};
use protocol::types::{Address, Hash, Hex, ServiceContext};

use crate::types::{KeccakPayload, KeccakResponse, SigVerifyPayload, SigVerifyResponse};


pub struct UtilService<SDK> {
    sdk: SDK,
}

#[service]
impl<SDK: ServiceSDK> UtilService<SDK> {
    pub fn new(mut sdk: SDK) -> Self {
        Self { sdk }
    }

    #[cycles(100_00)]
    #[read]
    fn keccak256(
        &self,
        ctx: ServiceContext,
        payload: KeccakPayload,
    ) -> ServiceResponse<KeccakResponse> {
        let keccak = HasherKeccak::new();

        let data = hex::decode(payload.hex_str.as_string_trim0x()).unwrap();
        let hash_res = keccak.digest(data.as_slice());

        return match Hash::from_bytes(Bytes::from(hash_res)) {
            Ok(res) => {
                let response = KeccakResponse {
                    result: res,
                };
                ServiceResponse::<KeccakResponse>::from_succeed(response)
            }

            _ => ServiceResponse::<KeccakResponse>::from_error(
                101,
                "data not valid".to_owned(),
            )
        };
    }

    #[cycles(100_00)]
    #[read]
    fn verify(
        &self,
        ctx: ServiceContext,
        payload: SigVerifyPayload,
    ) -> ServiceResponse<SigVerifyResponse> {
        let data_sig = hex::decode(payload.sig.as_string_trim0x()).unwrap();
        let data_pk = hex::decode(payload.pub_key.as_string_trim0x()).unwrap();
        let data_hash = payload.hash.as_bytes();

        let response = SigVerifyResponse {
            is_ok: Secp256k1::verify_signature(data_hash.as_ref(), data_sig.as_slice(),data_pk.as_slice()).is_ok()
        };

        ServiceResponse::<SigVerifyResponse>::from_succeed(response)
    }
}
