#!/bin/bash
set -ev

cargo sweep --version || cargo install --git https://github.com/holmgr/cargo-sweep --rev 3e98dbf7e4ddf1e07dd2526a803c501fd549da75

if [ "$FMT" = true ]; then
  cargo fmt --version || rustup component add rustfmt
fi

if [ "$CHECK" = true ]; then
  cargo clippy --version || rustup component add clippy
fi
