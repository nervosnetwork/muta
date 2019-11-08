use protocol::types::{Hash, UserAddress};
use rand::{rngs::OsRng, RngCore};
use tentacle_secio::SecioKeyPair;

pub fn main() {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);

    let seckey = Hash::digest(seed.as_ref().into());
    let keypair = SecioKeyPair::secp256k1_raw_key(seckey.as_bytes()).expect("secp256k1 keypair");
    let pubkey = keypair.to_public_key().inner();
    let user_addr = UserAddress::from_pubkey_bytes(pubkey.into()).expect("user addr");

    println!("seckey hex: {:?}", seckey.as_hex());
    println!(
        "pubkey hex: {:?}",
        hex::encode(keypair.to_public_key().inner())
    );
    println!("user addr hex: {}", user_addr.as_hex());
}
