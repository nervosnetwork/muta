FROM ubuntu:18.04
LABEL maintainer="yejiayu.fe@gmail.com"

COPY target/release/examples/muta-chain .
COPY devtools/chain/config.toml devtools/chain/config.toml
COPY devtools/chain/genesis.toml devtools/chain/genesis.toml

EXPOSE 1337 8000
CMD ["./muta-chain"]
