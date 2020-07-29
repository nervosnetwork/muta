use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use bech32::{self, ToBase32};
use hasher::{Hasher, HasherKeccak};

const DEFAULT_ADDRESS_HRP: &str = "muta";
const DEFAULT_ADDRESS_HRP_FILENAME: &str = "address_hrp.rs";

fn main() {
    let address_hrp = env::var("ADDRESS_HRP").unwrap_or_else(|_| DEFAULT_ADDRESS_HRP.to_owned());

    // Verify hrp
    let hash = HasherKeccak::new().digest(b"hello muta");
    assert_eq!(hash.len(), 32);

    let bytes = &hash[12..];
    assert_eq!(bytes.len(), 20);

    bech32::encode(&address_hrp, bytes.to_base32()).unwrap();

    // Generate address hrp file
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join(DEFAULT_ADDRESS_HRP_FILENAME);
    let mut file = BufWriter::new(File::create(&path).unwrap());

    write!(&mut file, "\"{}\"", address_hrp).unwrap();
}
