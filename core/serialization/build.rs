const INCLUDES: &'static [&'static str] = &["proto/"];
const INPUTS: &'static [&'static str] = &[
    "proto/block.proto",
    "proto/transaction.proto",
    "proto/receipt.proto",
];

fn main() {
    println!("cargo:rerun-if-changed=proto");
    prost_build::compile_protos(INPUTS, INCLUDES).unwrap();
}
