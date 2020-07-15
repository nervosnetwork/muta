#[macro_use]
extern crate clap;

use std::convert::TryFrom;
use std::default::Default;

use clap::App;
use ophelia::{PublicKey, ToBlsPublicKey};
use ophelia_bls_amcl::BlsPrivateKey;
use protocol::types::{Address, Hash};
use protocol::{Bytes, BytesMut};
use rand::distributions::Alphanumeric;
use rand::Rng;
use rand::{rngs::OsRng, RngCore};
use serde::Serialize;
use tentacle_secio::SecioKeyPair;

#[derive(Default, Serialize, Debug)]
struct Keypair {
    pub index:          usize,
    pub private_key:    String,
    pub public_key:     String,
    pub address:        String,
    pub bls_public_key: String,
}

#[derive(Default, Serialize, Debug)]
struct Output {
    pub common_ref: String,
    pub keypairs:   Vec<Keypair>,
}

#[allow(clippy::needless_range_loop)]
pub fn main() {
    let yml = load_yaml!("keypair.yml");
    let m = App::from(yml).get_matches();
    let number = value_t!(m, "number", usize).unwrap();
    let priv_keys = values_t!(m.values_of("private_keys"), String).unwrap_or_default();
    let len = priv_keys.len();
    if len > number {
        panic!("private keys length can not be larger than number");
    }

    let common_ref_encoded = value_t!(m, "common_ref", String).unwrap();
    let common_ref = if common_ref_encoded.is_empty() {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(10)
            .collect::<String>()
    } else {
        String::from_utf8(
            hex::decode(common_ref_encoded).expect("common_ref should be a hex string"),
        )
        .expect("common_ref should be a valid utf8 string")
    };

    let mut output = Output {
        common_ref: add_0x(hex::encode(common_ref.clone())),
        keypairs:   vec![],
    };

    for i in 0..number {
        let mut k = Keypair::default();
        let seckey = if i < len {
            Bytes::from(hex::decode(&priv_keys[i]).expect("decode hex private key"))
        } else {
            let mut seed = [0u8; 32];
            OsRng.fill_bytes(&mut seed);
            Hash::digest(BytesMut::from(seed.as_ref()).freeze()).as_bytes()
        };
        let keypair = SecioKeyPair::secp256k1_raw_key(seckey.as_ref()).expect("secp256k1 keypair");
        let pubkey = keypair.to_public_key().inner();
        let user_addr = Address::from_pubkey_bytes(pubkey.into()).expect("user addr");

        k.private_key = add_0x(hex::encode(seckey.as_ref()));
        k.public_key = add_0x(hex::encode(keypair.to_public_key().inner()));
        k.address = add_0x(user_addr.as_hex());

        let priv_key =
            BlsPrivateKey::try_from([&[0u8; 16], seckey.as_ref()].concat().as_ref()).unwrap();
        let pub_key = priv_key.pub_key(&common_ref.as_str().into());
        k.bls_public_key = add_0x(hex::encode(pub_key.to_bytes()));
        k.index = i + 1;
        output.keypairs.push(k);
    }
    let output_str = serde_json::to_string_pretty(&output).unwrap();
    println!("{}", output_str);
}

fn add_0x(s: String) -> String {
    "0x".to_owned() + &s
}
