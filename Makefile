ERBOSE := $(if ${CI},--verbose,)

COMMIT := $(shell git rev-parse --short HEAD)

ifneq ("$(wildcard /usr/lib/librocksdb.so)","")
	SYS_LIB_DIR := /usr/lib
else ifneq ("$(wildcard /usr/lib64/librocksdb.so)","")
	SYS_LIB_DIR := /usr/lib64
else
	USE_SYS_ROCKSDB :=
endif

USE_SYS_ROCKSDB :=
SYS_ROCKSDB := $(if ${USE_SYS_ROCKSDB},ROCKSDB_LIB_DIR=${SYS_LIB_DIR},)

CARGO := env ${SYS_ROCKSDB} cargo

test:
	${CARGO} test ${VERBOSE} --all -- --skip trust_metric --nocapture

doc:
	cargo doc --all --no-deps

doc-deps:
	cargo doc --all

# generate GraphQL API documentation
doc-api:
	bash docs/build/gql_api.sh

check:
	${CARGO} check ${VERBOSE} --all

build:
	${CARGO} build ${VERBOSE} --release

prod-muta-chain:
	${CARGO} build ${VERBOSE} --release --example muta-chain

fmt:
	cargo fmt ${VERBOSE} --all -- --check

clippy:
	${CARGO} clippy ${VERBOSE} --all --all-targets --all-features -- \
		-D warnings -D clippy::clone_on_ref_ptr -D clippy::enum_glob_use


ci: fmt clippy test

info:
	date
	pwd
	env

e2e-test:
	cargo build --example muta-chain
	rm -rf ./devtools/chain/data
	./target/debug/examples/muta-chain -c ./devtools/chain/config.toml -g ./devtools/chain/genesis.toml > /tmp/log 2>&1 &
	cd tests/e2e && yarn && ./wait-for-it.sh -t 300 localhost:8000 -- yarn run test
	pkill -2 muta-chain

byz-test:
	cargo build --example muta-chain
	cargo build --example byzantine_node
	rm -rf ./devtools/chain/data
	CONFIG=./examples/config-1.toml GENESIS=./examples/genesis.toml ./target/debug/examples/muta-chain > /tmp/log 2>&1 &
	CONFIG=./examples/config-2.toml GENESIS=./examples/genesis.toml ./target/debug/examples/muta-chain > /tmp/log 2>&1 &
	CONFIG=./examples/config-3.toml GENESIS=./examples/genesis.toml ./target/debug/examples/muta-chain > /tmp/log 2>&1 &
	CONFIG=./examples/config-4.toml GENESIS=./examples/genesis.toml ./target/debug/examples/byzantine_node > /tmp/log 2>&1 &
	cd byzantine/tests && yarn && ../../tests/e2e/wait-for-it.sh -t 300 localhost:8000 -- yarn run test
	pkill -2 muta-chain byzantine_node

e2e-test-via-docker:
	docker-compose -f tests/e2e/docker-compose-e2e-test.yaml up --exit-code-from e2e-test --force-recreate

# For counting lines of code
stats:
	@cargo count --version || cargo +nightly install --git https://github.com/kbknapp/cargo-count
	@cargo count --separator , --unsafe-statistics

# Use cargo-audit to audit Cargo.lock for crates with security vulnerabilities
# expecting to see "Success No vulnerable packages found"
security-audit:
	@cargo audit --version || cargo install cargo-audit
	@cargo audit

.PHONY: build prod prod-test
.PHONY: fmt test clippy doc doc-deps doc-api check stats
.PHONY: ci info security-audit
