FROM huwenchao/muta-docker-builder:latest as cargo-build
WORKDIR /code
COPY . .
RUN cargo build

FROM huwenchao/muta-docker-runner:latest
WORKDIR /app
COPY --from=cargo-build /code/target/debug/muta-chain .
COPY ./devtools/chain/config.toml ./devtools/chain/config.toml
COPY ./devtools/chain/genesis.json ./devtools/chain/genesis.json
CMD ["./muta-chain"]

