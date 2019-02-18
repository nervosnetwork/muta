#[cfg(feature = "no-grpc")]
use protobuf_codegen_pure;
#[cfg(feature = "with-grpc")]
use protoc_rust_grpc;

const OUT_DIR: &'static str = "src";
const INCLUDES: &'static [&'static str] = &["./proto"];
const INPUT: &'static [&'static str] = &[
    "./proto/chain.proto",
    "./proto/common.proto",
    "./proto/consensus.proto",
    "./proto/executor.proto",
    "./proto/network.proto",
    "./proto/pool.proto",
    "./proto/sync.proto",
    "./proto/blockchain.proto",
];

fn main() {
    #[cfg(feature = "with-grpc")]
    protoc_rust_grpc::run(protoc_rust_grpc::Args {
        out_dir: OUT_DIR,
        includes: INCLUDES,
        input: INPUT,
        rust_protobuf: true,
        ..Default::default()
    })
    .expect("protoc-rust-grpc");

    #[cfg(feature = "no-grpc")]
    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: OUT_DIR,
        includes: INCLUDES,
        input: INPUT,
        customize: protobuf_codegen_pure::Customize {
            ..Default::default()
        },
    })
    .expect("protobuf-codegen-pure");
}
