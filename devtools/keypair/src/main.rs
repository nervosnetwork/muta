use rand::{rngs::OsRng, thread_rng, RngCore};
use tentacle_secio::SecioKeyPair;

use common_crypto::{BlsPrivateKey, PrivateKey, PublicKey, ToBlsPublicKey};
use protocol::types::{Hash, UserAddress};
use protocol::BytesMut;

pub fn main() {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);

    let seckey = Hash::digest(BytesMut::from(seed.as_ref()).freeze());
    let keypair = SecioKeyPair::secp256k1_raw_key(seckey.as_bytes()).expect("secp256k1 keypair");
    let pubkey = keypair.to_public_key().inner();
    let user_addr = UserAddress::from_pubkey_bytes(pubkey.into()).expect("user addr");

    println!("seckey hex: {:?}", seckey.as_hex());
    println!(
        "pubkey hex: {:?}",
        hex::encode(keypair.to_public_key().inner())
    );
    println!("user addr hex: {}", user_addr.as_hex());
    println!("================================================================");

    let priv_key_1 = BlsPrivateKey::generate(&mut thread_rng());
    let pub_key_1 = priv_key_1.pub_key(&"muta".into());
    let priv_key_2 = BlsPrivateKey::generate(&mut thread_rng());
    let pub_key_2 = priv_key_2.pub_key(&"muta".into());
    let priv_key_3 = BlsPrivateKey::generate(&mut thread_rng());
    let pub_key_3 = priv_key_3.pub_key(&"muta".into());
    let priv_key_4 = BlsPrivateKey::generate(&mut thread_rng());
    let pub_key_4 = priv_key_4.pub_key(&"muta".into());

    println!("private_key_1: {:?}", hex::encode(priv_key_1.to_bytes()));
    println!("private_key_2: {:?}", hex::encode(priv_key_2.to_bytes()));
    println!("private_key_3: {:?}", hex::encode(priv_key_3.to_bytes()));
    println!("private_key_4: {:?}", hex::encode(priv_key_4.to_bytes()));
    println!("public_key_1: {:?}", hex::encode(pub_key_1.to_bytes()));
    println!("public_key_2: {:?}", hex::encode(pub_key_2.to_bytes()));
    println!("public_key_3: {:?}", hex::encode(pub_key_3.to_bytes()));
    println!("public_key_4: {:?}", hex::encode(pub_key_4.to_bytes()));
}
