#!/bin/bash
set -ev

sccache --version || env RUSTC_WRAPPER= cargo install sccache
curl -sL https://deb.nodesource.com/setup_12.x | sudo -E bash -
sudo apt install -y nodejs yarn

if [ "$FMT" = true ]; then
  cargo fmt --version || rustup component add rustfmt
fi

if [ "$CHECK" = true ]; then
  cargo clippy --version || rustup component add clippy
fi
