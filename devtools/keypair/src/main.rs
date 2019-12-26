use rand::{rngs::OsRng, thread_rng, RngCore};
use tentacle_secio::SecioKeyPair;

use common_crypto::{BlsPrivateKey, PrivateKey, PublicKey, ToBlsPublicKey};
use protocol::types::{Address, Hash};
use protocol::BytesMut;

pub fn main() {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);

    let seckey = Hash::digest(BytesMut::from(seed.as_ref()).freeze());
    let keypair = SecioKeyPair::secp256k1_raw_key(seckey.as_bytes()).expect("secp256k1 keypair");
    let pubkey = keypair.to_public_key().inner();
    let user_addr = Address::from_pubkey_bytes(pubkey.into()).expect("user addr");

    println!("seckey hex: {:?}", seckey.as_hex());
    println!(
        "pubkey hex: {:?}",
        hex::encode(keypair.to_public_key().inner())
    );
    println!("user addr hex: {}", user_addr.as_hex());
    println!("================================================================");

    let n: usize = ::std::env::args()
        .last()
        .unwrap()
        .parse()
        .expect("argument error");
    let common_ref = "muta";
    println!("common ref: {:?}", hex::encode(common_ref.as_bytes()));

    for i in 0..n {
        let priv_key = BlsPrivateKey::generate(&mut thread_rng());
        let pub_key = priv_key.pub_key(&common_ref.into());
        println!(
            "bls private key {}: {:?}",
            i + 1,
            hex::encode(priv_key.to_bytes())
        );
        println!(
            "bls public key {}: {:?}",
            i + 1,
            hex::encode(pub_key.to_bytes())
        );
    }
}
