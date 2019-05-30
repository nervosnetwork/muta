# muta

[![Build Status](https://travis-ci.com/cryptape/muta.svg?token=e7nTwk1GkUYrpv8hmrt9&branch=master)](https://travis-ci.com/cryptape/muta)

[![CircleCI](https://circleci.com/gh/cryptape/muta/tree/master.svg?style=svg)](https://circleci.com/gh/cryptape/muta/tree/master)

"muta" is just a project code name.

## Compile and Run

You can download our [binary distribution[TODO]]() directly or compile it from source.

The first step to compile muta is to install rust. Generally speaking, you'll need an Internet connection to run the commands in this section, as we'll be downloading Rust from the Internet.

```sh
$ curl https://sh.rustup.rs -sSf | sh
```

You can get more infomation from [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).

And the next step, also the only step:

```sh
$ git clone https://github.com/cryptape/muta.git
$ cd muta
$ cargo run -- init
```

If everything goes well, you’ll see this appear:

```
[2019-05-30T02:54:42Z INFO  muta] Go with config...
```

The develop chain is worked on **LOCAL** and **SINGLE NODE**.

## The develop config and genesis account

In order to facilitate development, we set the default development configuration, which located in `./devtools/chain/config.toml` and `./devtools/chain/genesis.json`. What needs special explanation is that there are four **BUILD IN** account, each of them has about 1000 muta coin. All information is in `./devtools/chain`, so, if you are interested in develop, you can study it.

## Documentations

- [Four nodes](./docs/four_nodes.md)

## License

This project is free and open source software distributed under the terms of both the [MIT License][lm] and the [Apache License 2.0][la].

[lm]: docs/LICENSE-MIT
[la]: docs/LICENSE-APACHE

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
