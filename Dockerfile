FROM nervos/muta:build as builder
WORKDIR /code
COPY . .
RUN cargo build --release --example muta-chain

FROM nervos/muta:run
WORKDIR /app
COPY ./devtools/chain/config.toml ./devtools/chain/config.toml
COPY ./devtools/chain/genesis.toml ./devtools/chain/genesis.toml
COPY --from=builder /code/target/release/examples/muta-chain .
EXPOSE 1337 8000
CMD ["./muta-chain"]
