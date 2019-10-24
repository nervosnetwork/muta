FROM huwenchao/muta-docker-builder:latest as cargo-build
WORKDIR /code
COPY . .
RUN cargo build --release

FROM huwenchao/muta-docker-runner:latest
WORKDIR /app
COPY --from=cargo-build /code/target/release/muta-chain .
COPY ./devtools/chain/config.toml ./devtools/chain/config.toml
COPY ./devtools/chain/genesis.json ./devtools/chain/genesis.json
EXPOSE 1337 8000
CMD ["./muta-chain"]

