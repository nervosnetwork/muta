#!/bin/bash
set -ev

sccache --version || env RUSTC_WRAPPER= cargo install sccache

if [ "$FMT" = true ]; then
  cargo fmt --version || rustup component add rustfmt
fi

if [ "$CHECK" = true ]; then
  cargo clippy --version || rustup component add clippy
fi
