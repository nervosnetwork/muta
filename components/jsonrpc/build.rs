const INCLUDES: &'static [&'static str] = &["proto/"];
const INPUTS: &'static [&'static str] = &["proto/blockchain.proto"];

fn main() {
    println!("cargo:rerun-if-changed=proto");
    prost_build::compile_protos(INPUTS, INCLUDES).unwrap();
}
