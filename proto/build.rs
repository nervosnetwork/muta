const INCLUDES: &'static [&'static str] = &["proto/"];
const INPUTS: &'static [&'static str] = &[
    "proto/chain.proto",
    "proto/common.proto",
    "proto/consensus.proto",
    "proto/executor.proto",
    "proto/network.proto",
    "proto/pool.proto",
    "proto/sync.proto",
    "proto/blockchain.proto",
];

fn main() {
    println!("cargo:rerun-if-changed=proto");
    prost_build::compile_protos(INPUTS, INCLUDES).unwrap();
}
