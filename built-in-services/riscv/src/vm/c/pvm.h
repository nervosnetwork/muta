#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#ifndef _PVM_H
#define _PVM_H

static inline long __internal_syscall(long n, long _a0, long _a1, long _a2,
                                      long _a3, long _a4, long _a5) {
  register long a0 asm("a0") = _a0;
  register long a1 asm("a1") = _a1;
  register long a2 asm("a2") = _a2;
  register long a3 asm("a3") = _a3;
  register long a4 asm("a4") = _a4;
  register long a5 asm("a5") = _a5;
  register long syscall_id asm("a7") = n;
  asm volatile("scall"
               : "+r"(a0)
               : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a5), "r"(syscall_id));
  return a0;
}

#define syscall(n, a, b, c, d, e, f)                                           \
  __internal_syscall(n, (long)(a), (long)(b), (long)(c), (long)(d), (long)(e), \
                     (long)(f))

#define SYSCODE_DEBUG 2000
#define SYSCODE_LOAD_ARGS 2001
#define SYSCODE_RET 2002
#define SYSCODE_ASSERT 2003

#define SYSCODE_CYCLE_LIMIT 3000
#define SYSCODE_IS_INIT 3001
#define SYSCODE_ORIGIN 3002
#define SYSCODE_CALLER 3003
#define SYSCODE_ADDRESS 3004
#define SYSCODE_BLOCK_HEIGHT 3005
#define SYSCODE_CYCLE_USED 3006
#define SYSCODE_CYCLE_PRICE 3007
#define SYSCODE_EXTRA 3008
#define SYSCODE_TIMESTAMP 3009
#define SYSCODE_EMIT_EVENT 3010
#define SYSCODE_TX_HASH 3011
#define SYSCODE_TX_NONCE 3012

#define SYSCODE_GET_STORAGE 4000
#define SYSCODE_SET_STORAGE 4001
#define SYSCODE_CONTRACT_CALL 4002
#define SYSCODE_SERVICE_CALL 4003

/*
 * Function pvm_debug accepts a string that contains the text to be written to
 * stdout(It depends on the VM).
 *
 * Params:
 *   format[in]: same as the standard C function `printf()`
 *
 * Return:
 *   code: 0(success)
 *
 * Example:
 *   pvm_debug("Hello World!");
 */
int pvm_debug(const char *s) {
  return syscall(SYSCODE_DEBUG, s, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_assert accepts bool statement and a assertion message that
 * contains the text to be written to stdout(It depends on the VM). If bool
 * statement evaluates to false, execution will be aborted. Assertion message
 * only output in debug mode.
 *
 * Params:
 *   statement[in]: bool statement
 *   msg[in]: same as the standard C function `printf()`
 *
 * Example:
 *   pvm_assert(2 > 1, "1 should never bigger than 2");
 */
void pvm_assert(int statement, const char *msg) {
  syscall(SYSCODE_ASSERT, statement, msg, 0, 0, 0, 0);
}

/*
 * Function pvm_load_args load contract invocation arguments.
 *
 * Params:
 *   data[out]: pointer to data for loaded args to write
 *   size[out]: size of loaded args
 *
 * Return:
 *   code: 0(success)
 *
 * Example:
 *   char data[2048];
 *   uint64_t size = 0;
 *   pvm_load_args(data, &size);
 */
int pvm_load_args(uint8_t *data, uint64_t *size) {
  return syscall(SYSCODE_LOAD_ARGS, data, size, 0, 0, 0, 0);
}

/*
 * Function ret returns any bytes to host, as the output of the current
 * contract.
 *
 * Params:
 *   data[in]: point to data to returen
 *   size[in]: size of the data
 *
 * Return:
 *   code: 0(success)
 *
 * Example:
 *   const char *ret = "return message";
 *   pvm_ret((uint8_t *)ret, strlen(ret));
 *
 * Note: This syscall(s) only allowed to call once. If called it multiple times,
 * the last call will replace the previous call.
 */
int pvm_ret(const uint8_t *data, size_t size) {
  return syscall(SYSCODE_RET, data, size, 0, 0, 0, 0);
}

/*
 * Function pvm_cycle_limit returns block cycle limit.
 *
 * Example:
 *   uint64_t cycle_limit = pvm_cycle_limit();
 */
uint64_t pvm_cycle_limit() {
  return syscall(SYSCODE_CYCLE_LIMIT, 0, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_cycle_used returns execution used cycle.
 *
 * Example:
 *   uint64_t cycle_used = pvm_cycle_used();
 */
uint64_t pvm_cycle_used() {
  return syscall(SYSCODE_CYCLE_USED, 0, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_cycle_price returns cycle price.
 *
 * Example:
 *   uint64_t cycle_price = pvm_cycle_price();
 */
uint64_t pvm_cycle_price() {
  return syscall(SYSCODE_CYCLE_PRICE, 0, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_origin loads current origin address.
 *
 * Params:
 *   addr[out]: pointer to buffer for loaded origin address to write
 *
 * Return:
 *   code: 0(success)
 *
 * Example:
 *   uint8_t addr[50];
 *   pvm_origin(addr);
 */
int pvm_origin(uint8_t *addr) {
  return syscall(SYSCODE_ORIGIN, addr, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_caller loads current caller address.
 *
 * Params:
 *   addr[out]: pointer to bufer for loaded caller address to write
 *
 * Return:
 *   code: 0(success)
 *
 * Example:
 *   uint8_t addr[50];
 *   pvm_caller(addr);
 */
int pvm_caller(uint8_t *addr) {
  return syscall(SYSCODE_CALLER, addr, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_address load this contract address.
 *
 * Params:
 *   addr[out]: pointer to buffer for loaded contract address to write
 *
 *  Return:
 *    code: 0(success)
 *
 *  Example:
 *    uint8_t addr[50];
 *    pvm_adress(addr);
 */
int pvm_address(uint8_t *addr) {
  return syscall(SYSCODE_ADDRESS, addr, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_is_init returns whether this contract is initialized. Contract
 * can be deploy with init args.
 *
 * Example:
 *   if (pvm_is_init()) {
 *     // do something
 *   }
 */
int pvm_is_init() { return syscall(SYSCODE_IS_INIT, 0, 0, 0, 0, 0, 0); }

/*
 * Function pvm_block_height returns current block height.
 *
 * Exmaple:
 *   uint64_t block_height = pvm_block_height();
 */
uint64_t pvm_block_height() {
  return syscall(SYSCODE_BLOCK_HEIGHT, 0, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_extra loads extra data.
 *
 * Params:
 *   extra[out]: pointer to buffer for loaded extra to write
 *   extra_sz[out]: size of extra data if there is one
 *
 * Return:
 *   code: 0(success), 1(no extra data)
 *
 * Example:
 *   uint8_t extra[2048];
 *   uint64_t extra_sz;
 *   pvm_extra(extra, &extra_sz);
 */
int pvm_extra(uint8_t *extra, uint64_t *extra_sz) {
  return syscall(SYSCODE_EXTRA, extra, extra_sz, 0, 0, 0, 0);
}

/*
 * Function pvm_timestamp returns execution's timestamp. It's seconds since
 * 1970-01-01 00:00:00 UTC.
 *
 * Example:
 *   uint64_t timestamp = pvm_timestamp();
 */
uint64_t pvm_timestamp() {
  return syscall(SYSCODE_TIMESTAMP, 0, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_emit_event emit event message string. Message is UTF-8 encoded.
 *
 * Params:
 *   msg[in]: a pointer to msg to emit
 *   msg_sz[in]: size of message buffer
 *
 * Return:
 *   code: 0(success), 1(invalid utf8)
 *
 * Example:
 *   const char *msg = "{ \"msg\": \"test event\" }";
 *   pvm_emit_event((uint8_t *)msg, strlen(msg));
 */
int pvm_emit_event(const uint8_t *msg, uint64_t msg_sz) {
  return syscall(SYSCODE_EMIT_EVENT, msg, msg_sz, 0, 0, 0, 0);
}

/*
 * Function pvm_tx_hash loads transaction hash.
 *
 * Params:
 *   addr[out]: pointer to buffer for loaded tx hash to write
 *
 * Return:
 *   code: 0(success) 1(no tx hash)
 *
 * Example:
 *   uint8_t addr[50];
 *   pvm_tx_hash(addr);
 */
int pvm_tx_hash(uint8_t *addr) {
  return syscall(SYSCODE_TX_HASH, addr, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_nonce loads transaction nonce hash.
 *
 * Params:
 *   addr[out]: pointer to buffer for loaded nonce to write
 *
 * Return:
 *   code: 0(success) 1(no nonce)
 *
 * Example:
 *   uint8_t nonce[50];
 *   pvm_tx_nonce(nonce);
 */
int pvm_tx_nonce(uint8_t *addr) {
  return syscall(SYSCODE_TX_NONCE, addr, 0, 0, 0, 0, 0);
}

/*
 * Function pvm_get_storage load value from contract state.
 *
 * Params:
 *   k[in]: pointer to key
 *   k_size[in]: size of key
 *
 *   v[out]: pointer to buffer for loaded value to write
 *   v_size[out]: size of val
 *
 * Return:
 *   code: 0(success) 1(key not found)
 *
 * Example:
 *   const char *key = "cyber";
 *   uint8_t val[2048];
 *   uint64_t val_sz;
 *   pvm_get_storage((uint8_t *)key, strlen(key), val, &val_sz);
 */
int pvm_get_storage(const uint8_t *k, uint64_t k_size, uint8_t *v,
                    uint64_t *v_size) {
  return syscall(SYSCODE_GET_STORAGE, k, k_size, v, v_size, 0, 0);
}

/*
 * Function pvm_set_storage save value to contract state using given key.
 *
 * Params:
 *   k[in]: pointer to key
 *   k_size[in]: size of key
 *
 *   v[in]: pointer to value
 *   v_size[in]: size of val
 *
 * Return:
 *   code: 0(success)
 *
 * Example:
 *   const char *key = "cyber";
 *   const char *val = "punk"
 *   pvm_set_storage((uint8_t *)key, strlen(key), (uint8_t *)val, strlen(val));
 */
int pvm_set_storage(const uint8_t *k, uint64_t k_size, const uint8_t *v,
                    uint64_t v_size) {
  return syscall(SYSCODE_SET_STORAGE, k, k_size, v, v_size, 0, 0);
}

/*
 * Function pvm_contract_call invokes a contract located at given address.
 *
 * Params:
 *   addr[in]: pointer to contract address
 *   args[in]: pointer to invocation args
 *   args_size[in]: size of args
 *
 *   ret[out]: pointer to a buffer for invocation result to write
 *   ret_size[out]: size of result
 *
 * Return:
 *   code: 0(success)
 *
 * Example:
 *   uint8_t *ctr_addr = xxxx; // target contract address
 *   const char *args = "{\"method\": \"test_contract_call\"}"; // json
 *   uint8_t ret[2048];
 *   uint64_t ret_size;
 *   pvm_contract_call(ctr_addr, (uint8_t *)args, strlen(args), ret, &ret_size);
 */
int pvm_contract_call(const uint8_t *addr, const uint8_t *args,
                      uint64_t args_size, uint8_t *ret, uint64_t *ret_size) {
  return syscall(SYSCODE_CONTRACT_CALL, addr, args, args_size, ret, ret_size,
                 0);
}

/*
 * Function pvm_service_call invokes a service method.
 *
 * Params:
 *   service[in]: target service name
 *   method[in]: target service's method name
 *   payload[in]: pointer to invocation payload
 *   payload_size[in]: size of payload
 *
 *   ret[out]: pointer to buffer for invocation result to write
 *   ret_size[out]: size of result
 *
 * Return:
 *   code: 0(success)
 *
 * Example:
 *   const char *service = "riscv";
 *   const char *method = "exec";
 *   const char *payload = "{ \
 *      \"address\": \"xxxx\", \
 *      \"args\": { \
 *         \"method\": \"test_method\", \
 *      } \
 *   }";
 *   uint8_t ret[2048];
 *   uint64_t ret_size;
 *   pvm_service_call(service, method, (uint8_t *)payload, strlen(payload), ret,
 * &ret_size);
 */
int pvm_service_call(const char *service, const char *method,
                     const uint8_t *payload, uint64_t payload_size,
                     uint8_t *ret, uint64_t *ret_size) {
  return syscall(SYSCODE_SERVICE_CALL, service, method, payload, payload_size,
                 ret, ret_size);
}

#endif
