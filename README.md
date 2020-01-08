# Muta

[![Build Status](https://travis-ci.com/nervosnetwork/muta.svg?branch=master)](https://travis-ci.com/nervosnetwork/muta)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Minimum rustc version](https://img.shields.io/badge/rustc-nightly-informational.svg)](https://github.com/nervosnetwork/muta/blob/master/rust-toolchain)

Muta is a high-performance blockchain framework.

## Documentations

- [Layout](docs/layout.md)
- [How to develop a core crate](docs/how_to_deploy_a_core_crate.md)

## Compile and Run

The first step to compile muta is to install rust. Generally speaking, you'll need an Internet connection to run the commands in this section, as we'll be downloading Rust from the Internet.

```shell
$ curl https://sh.rustup.rs -sSf | sh
```

You can get more infomation from [here](https://www.rust-lang.org/tools/install).

And the next step, also the only step:

```shell
$ git clone https://github.com/nervosnetwork/muta.git
$ cd muta
$ cargo run --release --example muta-chain
```

You can also run muta chain with your custom config and gensis epoch:

```shell
$ CONFIG=$confit_path GENSIS=$gensis_path cargo run --release --example muta-chain
```

If everything goes well, you’ll see this appear:

```log
[2019-09-25T15:26:14Z INFO  muta] Go with config: Config { .. }
```

Go to [http://127.0.0.1:8000/graphiql](http://127.0.0.1:8000/graphiql) to communicate with the chain and read the documentations after the chain is running.

The develop chain is worked on **LOCAL** and **SINGLE NODE**.

## How to Contribute

The contribution workflow is described in [CONTRIBUTING.md](CONTRIBUTING.md), and security policy is described in [SECURITY.md](SECURITY.md).
