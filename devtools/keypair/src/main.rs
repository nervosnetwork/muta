#[macro_use]
extern crate clap;

use std::convert::TryFrom;
use std::default::Default;

use clap::App;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rand::{rngs::OsRng, RngCore};
use serde::Serialize;
use tentacle_secio::SecioKeyPair;

use common_crypto::{BlsPrivateKey, PublicKey, ToBlsPublicKey};
use protocol::types::{Address, Hash};
use protocol::BytesMut;

#[derive(Default, Serialize, Debug)]
struct Keypair {
    pub index:          u32,
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

pub fn main() {
    let yml = load_yaml!("keypair.yml");
    let m = App::from(yml).get_matches();
    let number = value_t!(m, "number", u32).unwrap();
    let common_ref = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .collect::<String>();

    let mut output = Output {
        common_ref: hex::encode(common_ref.clone()),
        keypairs:   vec![],
    };

    for i in 0..number {
        let mut k = Keypair::default();

        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);

        let seckey = Hash::digest(BytesMut::from(seed.as_ref()).freeze());
        let keypair =
            SecioKeyPair::secp256k1_raw_key(seckey.as_bytes()).expect("secp256k1 keypair");
        let pubkey = keypair.to_public_key().inner();
        let user_addr = Address::from_pubkey_bytes(pubkey.into()).expect("user addr");

        k.private_key = seckey.as_hex();
        k.public_key = hex::encode(keypair.to_public_key().inner());
        k.address = user_addr.as_hex();

        let priv_key =
            BlsPrivateKey::try_from([&[0u8; 16], seckey.as_bytes().as_ref()].concat().as_ref())
                .unwrap();
        let pub_key = priv_key.pub_key(&common_ref.as_str().into());
        k.bls_public_key = hex::encode(pub_key.to_bytes());
        k.index = i + 1;
        output.keypairs.push(k);
    }
    let output_str = serde_json::to_string_pretty(&output).unwrap();
    println!("{}", output_str);
}
