FROM nervos/muta:build as builder
WORKDIR /code
COPY . .
RUN cargo build --release

FROM nervos/muta:run
WORKDIR /app
COPY ./devtools/chain/config.toml ./devtools/chain/config.toml
COPY ./devtools/chain/genesis.json ./devtools/chain/genesis.json
COPY --from=builder /code/target/release/muta-chain .
EXPOSE 1337 8000
CMD ["./muta-chain"]

