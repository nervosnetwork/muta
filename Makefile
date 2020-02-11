ERBOSE := $(if ${CI},--verbose,)

COMMIT := $(shell git rev-parse --short HEAD)

ifneq ("$(wildcard /usr/lib/librocksdb.so)","")
	SYS_LIB_DIR := /usr/lib
else ifneq ("$(wildcard /usr/lib64/librocksdb.so)","")
	SYS_LIB_DIR := /usr/lib64
else
	USE_SYS_ROCKSDB :=
endif

SYS_ROCKSDB := $(if ${USE_SYS_ROCKSDB},ROCKSDB_LIB_DIR=${SYS_LIB_DIR},)

CARGO := env ${SYS_ROCKSDB} cargo

test:
	${CARGO} test ${VERBOSE} --all -- --nocapture

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

prod:
	${CARGO} build ${VERBOSE} --release

prod-test:
	${CARGO} test ${VERBOSE} --all -- --nocapture

fmt:
	cargo fmt ${VERBOSE} --all -- --check

clippy:
	${CARGO} clippy ${VERBOSE} --all --all-targets --all-features -- \
		-D warnings -D clippy::clone_on_ref_ptr -D clippy::enum_glob_use

ci: fmt clippy test
	git diff --exit-code Cargo.lock

info:
	date
	pwd
	env

docker-build:
	docker build -t mutadev/muta:build -f devtools/docker/dockerfiles/Dockerfile.muta_build .
	docker build -t mutadev/muta:run -f devtools/docker/dockerfiles/Dockerfile.muta_run .
	docker build -t mutadev/muta:${COMMIT} .

docker-push:
	docker tag nervos/muta:${COMMIT}  mutadev/muta:${COMMIT}
	docker push mutadev/muta:${COMMIT}

e2e-test:
	@echo "-----------------------------------------------------------------"
	@echo "run the commands below in another window first:                  "
	@echo "                                                                 "
	@echo "rm -rf ./target/tests/e2e/data && \                              "
	@echo "RUST_LOG=info,overlord=warn cargo run -- -c tests/e2e/config.toml"
	@echo "-----------------------------------------------------------------"
	cd tests/e2e && yarn && ./wait-for-it.sh -t 300 localhost:8000 -- yarn run test

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
.PHONY: duktape
.PHONY: libpvm.a

# For riscv service
TARGET := riscv64-unknown-elf
CC := $(TARGET)-gcc
LD := $(TARGET)-gcc
CFLAGS := -Os -DCKB_NO_MMU -D__riscv_soft_float -D__riscv_float_abi_soft
LDFLAGS := -lm -Wl,-static -fdata-sections -ffunction-sections -Wl,--gc-sections -Wl,-s
CURRENT_DIR := $(shell pwd)
DOCKER_BUILD := docker run --rm -it -v $(CURRENT_DIR):/src nervos/ckb-riscv-gnu-toolchain:xenial bash -c
TEST_SRC := $(CURRENT_DIR)/built-in-services/riscv/src/tests
RISCV_SRC := $(CURRENT_DIR)/built-in-services/riscv/src/vm/c
DUKTAPE_SRC := $(RISCV_SRC)/duktape
DEMO_SRC := $(CURRENT_DIR)/built-in-services/riscv/examples/dex

duktape: libpvm.a
	$(CC) -I$(DUKTAPE_SRC) -I$(RISCV_SRC) $(DUKTAPE_SRC)/duktape.c $(RISCV_SRC)/duktape_ee.c $(RISCV_SRC)/libpvm.a $(LDFLAGS) -o $(RISCV_SRC)/duktape_ee.bin

duktape_docker:
	$(DOCKER_BUILD) "cd /src && make duktape"

libpvm.a:
	$(CC) -I$(RISCV_SRC) -c $(RISCV_SRC)/pvm.c -o /tmp/pvm.o
	ar rcs $(RISCV_SRC)/libpvm.a /tmp/pvm.o

pvm_structs_test: libpvm.a
	$(CC) -I$(RISCV_SRC) $(TEST_SRC)/pvm_structs.c $(RISCV_SRC)/libpvm.a $(LDFLAGS) -o $(TEST_SRC)/pvm_structs.bin

riscv_demo: libpvm.a
	$(CC) -I$(RISCV_SRC) $(DEMO_SRC)/cJSON.h $(DEMO_SRC)/cJSON.c $(DEMO_SRC)/erc20.c $(RISCV_SRC)/libpvm.a $(LDFLAGS) -o $(DEMO_SRC)/erc20.bin
	$(CC) -I$(RISCV_SRC) $(DEMO_SRC)/cJSON.h $(DEMO_SRC)/cJSON.c $(DEMO_SRC)/dex.c $(RISCV_SRC)/libpvm.a $(LDFLAGS) -o $(DEMO_SRC)/dex.bin

pvm_docker:
	$(DOCKER_BUILD) "cd /src && make pvm_structs_test"

riscv_demo_docker:
	$(DOCKER_BUILD) "cd /src && make riscv_demo"
