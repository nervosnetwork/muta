#include "pvm.h"

void pvm_debug(const char *msg) { syscall(SYSCODE_DEBUG, msg, 0, 0, 0, 0, 0); }

void pvm_assert(int statement, const char *msg) {
  syscall(SYSCODE_ASSERT, statement, msg, 0, 0, 0, 0);
}

uint64_t pvm_load_args(uint8_t *data) {
  return syscall(SYSCODE_LOAD_ARGS, data, 0, 0, 0, 0, 0);
}

void pvm_ret(const uint8_t *data, size_t size) {
  syscall(SYSCODE_RET, data, size, 0, 0, 0, 0);
}

uint64_t pvm_cycle_limit() {
  return syscall(SYSCODE_CYCLE_LIMIT, 0, 0, 0, 0, 0, 0);
}

uint64_t pvm_cycle_used() {
  return syscall(SYSCODE_CYCLE_USED, 0, 0, 0, 0, 0, 0);
}

uint64_t pvm_cycle_price() {
  return syscall(SYSCODE_CYCLE_PRICE, 0, 0, 0, 0, 0, 0);
}

uint64_t pvm_origin(uint8_t *addr) {
  return syscall(SYSCODE_ORIGIN, addr, 0, 0, 0, 0, 0);
}

uint64_t pvm_caller(uint8_t *addr) {
  return syscall(SYSCODE_CALLER, addr, 0, 0, 0, 0, 0);
}

uint64_t pvm_address(uint8_t *addr) {
  return syscall(SYSCODE_ADDRESS, addr, 0, 0, 0, 0, 0);
}

int pvm_is_init() { return syscall(SYSCODE_IS_INIT, 0, 0, 0, 0, 0, 0); }

uint64_t pvm_block_height() {
  return syscall(SYSCODE_BLOCK_HEIGHT, 0, 0, 0, 0, 0, 0);
}

uint64_t pvm_extra(uint8_t *extra) {
  return syscall(SYSCODE_EXTRA, extra, 0, 0, 0, 0, 0);
}

uint64_t pvm_timestamp() {
  return syscall(SYSCODE_TIMESTAMP, 0, 0, 0, 0, 0, 0);
}

void pvm_emit_event(const uint8_t *msg, uint64_t msg_sz) {
  syscall(SYSCODE_EMIT_EVENT, msg, msg_sz, 0, 0, 0, 0);
}

uint64_t pvm_tx_hash(uint8_t *tx_hash) {
  return syscall(SYSCODE_TX_HASH, tx_hash, 0, 0, 0, 0, 0);
}

uint64_t pvm_tx_nonce(uint8_t *nonce) {
  return syscall(SYSCODE_TX_NONCE, nonce, 0, 0, 0, 0, 0);
}

uint64_t pvm_get_storage(const uint8_t *k, uint64_t k_size, uint8_t *v) {
  return syscall(SYSCODE_GET_STORAGE, k, k_size, v, 0, 0, 0);
}

void pvm_set_storage(const uint8_t *k, uint64_t k_size, const uint8_t *v,
                     uint64_t v_size) {
  syscall(SYSCODE_SET_STORAGE, k, k_size, v, v_size, 0, 0);
}

uint64_t pvm_contract_call(const uint8_t *addr, const uint8_t *args,
                           uint64_t args_size, uint8_t *ret) {
  return syscall(SYSCODE_CONTRACT_CALL, addr, args, args_size, ret, 0, 0);
}

uint64_t pvm_service_call(const char *service, const char *method,
                          const uint8_t *payload, uint64_t payload_size,
                          uint8_t *ret) {
  return syscall(SYSCODE_SERVICE_CALL, service, method, payload, payload_size,
                 ret, 0);
}
