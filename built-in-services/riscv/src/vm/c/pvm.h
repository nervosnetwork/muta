/**
 * @file pvm.h
 * @author muta@nervos.org
 * @brief pvm.h provides wrapper to riscv service syscalls
 */

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

/**
 * @brief print debug message
 *
 * Function pvm_debug accepts a string that contains the text to be written to
 * stdout(It depends on the VM).
 *
 * @code{.c}
 *   pvm_debug("Hello World!");
 * @endcode
 * @param msg[in]: debug message
 * @return Void
 * @throw IO(InvalidInput) if msg is null
 * @throw IO(InvalidData) if msg is invalid utf-8 string
 */
void pvm_debug(const char *msg);

/**
 * @brief assert statement, if false print given message
 *
 * Function pvm_assert accepts bool statement and a assertion message that
 * contains the text to be written to stdout(It depends on the VM). If bool
 * statement evaluates to false, execution will be aborted, <B>Unexpected</B>
 * error is thrown. Assertion message only output in debug mode. Message
 * pointer can be null.
 *
 * @code{.c}
 *   pvm_assert(2 > 1, "1 should never bigger than 2");
 * @endcode
 * @param statement[in]: bool statement
 * @param msg[in]: assert message
 * @return Void
 * @throw IO(InvalidData) if msg is invalid utf-8 string
 */
void pvm_assert(int statement, const char *msg);

/**
 * @brief load contract call arguments
 *
 * Function pvm_load_args load contract invocation arguments. if data pointer
 * is null, only args size is returned.
 *
 * @code{.c}
 *   char data[2048];
 *   uint64_t size = pvm_load_args(data);
 * @endcode
 * @param data[out]: pointer to data for loaded args to write
 * @return size of loaded args in bytes, if no args, return 0
 */
uint64_t pvm_load_args(uint8_t *data);

/**
 * @brief return data bytes as the output of contract.
 *
 * Function ret returns any bytes to host, as the output of the current
 * contract.
 *
 * @code{.c}
 *   const char *ret = "return message";
 *   pvm_ret((uint8_t *)ret, strlen(ret));
 * @endcode
 * @param data[in]: point to data to returen
 * @param size[in]: size of the data
 * @return Void
 * @throw IO(InvalidInput) if data pointer is null
 * @note This syscall(s) only allowed to call once. If called it multiple
 * times, the last call will replace the previous call.
 */
void pvm_ret(const uint8_t *data, size_t size);

/**
 * @brief block cycle limit
 *
 * Function pvm_cycle_limit returns block cycle limit.
 *
 * @code{.c}
 *   uint64_t cycle_limit = pvm_cycle_limit();
 * @endcode
 * @return block cycle limit
 */
uint64_t pvm_cycle_limit();

/**
 * @brief cycle used
 *
 * Function pvm_cycle_used returns execution used cycle.
 *
 * @code{.c}
 *   uint64_t cycle_used = pvm_cycle_used();
 * @endcode
 * @return cycle used
 */
uint64_t pvm_cycle_used();

/**
 * @brief cycle price
 *
 * Function pvm_cycle_price returns cycle price.
 *
 * @code{.c}
 *   uint64_t cycle_price = pvm_cycle_price();
 * @endcode
 * @return cycle price
 */
uint64_t pvm_cycle_price();

/**
 * @brief contract call origin address
 *
 * Function pvm_origin loads current origin address.
 *
 * @code{.c}
 *   uint8_t *addr = malloc(pvm_origin(NULL));
 *   pvm_origin(addr);
 * @endcode
 * @param addr[out]: pointer to buffer for loaded origin address to write
 * @return size of origin address in bytes
 */
uint64_t pvm_origin(uint8_t *addr);

/**
 * @brief contract caller address
 *
 * Function pvm_caller loads current caller address.
 *
 * @code{.c}
 *   uint8_t *addr = malloc(pvm_caller(NULL));
 *   pvm_caller(addr);
 * @endcode
 * @param addr[out]: pointer to bufer for loaded caller address to write
 * @return size of caller address in bytes
 */
uint64_t pvm_caller(uint8_t *addr);

/**
 * @brief contract address
 *
 * Function pvm_address load this contract address.
 *
 * @code{.c}
 *    uint8_t addr[50];
 *    pvm_adress(addr);
 * @endcode
 * @param addr[out]: pointer to buffer for loaded contract address to write
 * @return size of contract address in bytes
 */
uint64_t pvm_address(uint8_t *addr);

/**
 * @brief check whether contract is init
 *
 * Function pvm_is_init returns whether this contract is initialized. Contract
 * can be deploy with init args.
 *
 * @code{.c}
 *   if (pvm_is_init()) {
 *     // do something
 *   }
 * @endcode
 * @return 1 if contract is init
 */
int pvm_is_init();

/**
 * @brief block height
 *
 * Function pvm_block_height returns current block height.
 *
 * @code{.c}
 *   uint64_t block_height = pvm_block_height();
 * @endcode
 * @return block height
 */
uint64_t pvm_block_height();

/**
 * @brief load extra data
 *
 * Function pvm_extra loads extra data. For service call, extra data is
 * servical caller.
 *
 * @code{.c}
 *   uint8_t *extra = malloc(pvm_extra(NULL));
 *   pvm_extra(extra);
 * @endcode
 * @param extra[out]: pointer to buffer for loaded extra to write
 * @return size of extra data in bytes
 */
uint64_t pvm_extra(uint8_t *extra);

/**
 * @brief execution timestamp
 *
 * Function pvm_timestamp returns execution's timestamp. It's seconds since
 * 1970-01-01 00:00:00 UTC.
 *
 * @code{.c}
 *   uint64_t timestamp = pvm_timestamp();
 * @endcode
 * @return execution timestamp
 */
uint64_t pvm_timestamp();

/**
 * @brief emit event message
 *
 * Function pvm_emit_event emit event message string. Message is UTF-8 encoded.
 *
 * @code{.c}
 *   const char *msg = "{ \"msg\": \"test event\" }";
 *   pvm_emit_event((uint8_t *)msg, strlen(msg));
 * @endcode
 * @param msg[in]: a pointer to msg to emit
 * @throw IO(InvalidInput) if msg pointer is null
 * @throw IO(InvalidData) if msg is invalid utf-8 string
 */
void pvm_emit_event(const uint8_t *msg, uint64_t msg_sz);

/**
 * @brief load transaction hash
 *
 * Function pvm_tx_hash loads transaction hash.
 *
 * @code{.c}
 *   uint8_t *tx_hash = malloc(pvm_tx_hash(NULL));
 *   pvm_tx_hash(tx_hash);
 * @endcode
 * @param tx_hash[out]: pointer to buffer for loaded tx hash to write
 * @return size of transaction hash in bytes
 */
uint64_t pvm_tx_hash(uint8_t *tx_hash);

/**
 * @brief load transaction nonce hash
 *
 * Function pvm_nonce loads transaction nonce hash.
 *
 * @code{.c}
 *   uint8_t *nonce = malloc(pvm_nonce(NULL));
 *   pvm_tx_nonce(nonce);
 * @endcode
 * @param nonce[out]: pointer to buffer for loaded nonce to write
 * @return size of nonce hash in bytes
 */
uint64_t pvm_tx_nonce(uint8_t *nonce);

/**
 * @brief load value from contract state
 *
 * Function pvm_get_storage load value from contract state.
 *
 * @code{.c}
 *   const char *key = "cyber";
 *   uint8_t *val = pvm_get_storage((uint8_t *key), strlen(key), NULL);
 *   pvm_get_storage((uint8_t *)key, strlen(key), val);
 * @endcode
 * @param k[in]: pointer to key
 * @param k_size[in]: size of key
 * @param v[out]: pointer to buffer for loaded value to write
 * @return size of loaded value in bytes
 * @throw IO(InvalidInput) if k pointer is null or k_size is 0
 * @throw IO(Other) if fail to load value from state
 */
uint64_t pvm_get_storage(const uint8_t *k, uint64_t k_size, uint8_t *v);

/**
 * @brief save value to contract state
 *
 * Function pvm_set_storage save value to contract state using given key.
 *
 * @code{.c}
 *   const char *key = "cyber";
 *   const char *val = "punk"
 *   pvm_set_storage((uint8_t *)key, strlen(key), (uint8_t *)val, strlen(val));
 * @endcode
 * @param k[in]: pointer to key
 * @param k_size[in]: size of key
 * @param v[in]: pointer to value
 * @param v_size[in]: size of val
 * @return Void
 * @throw IO(InvalidInput) if k or v pointer is null
 * @throw IO(InvalidInput) if k_size is 0
 * @throw IO(Other) if fail to save value
 */
void pvm_set_storage(const uint8_t *k, uint64_t k_size, const uint8_t *v,
                     uint64_t v_size);

/**
 * @brief call a contract
 *
 * Function pvm_contract_call invokes a contract located at given address.
 *
 * @code{.c}
 *   uint8_t *ctr_addr = xxxx; // target contract address
 *   const char *args = "{\"method\": \"test_contract_call\"}"; // json
 *   uint8_t ret[2048];
 *   uint64_t size = pvm_contract_call(ctr_addr, (uint8_t *)args, strlen(args),
 *   ret);
 *   pvm_assert(2 > 1, "contract call success");
 * @endcode
 * @param addr[in]: pointer to contract address
 * @param args[in]: pointer to invocation args
 * @param args_size[in]: size of args in bytes
 * @param ret[out]: pointer to a buffer for invocation result to write
 * @return size of result in bytes
 * @throw IO(InvalidInput) if address pointer is null
 * @throw IO(InvalidData) if address is invalid address
 * @throw IO(Other) if contract call failure
 */
uint64_t pvm_contract_call(const uint8_t *addr, const uint8_t *args,
                           uint64_t args_size, uint8_t *ret);

/**
 * @brief call a service
 *
 * Function pvm_service_call invokes a service method.
 *
 * @code{.c}
 *   const char *service = "riscv";
 *   const char *method = "exec";
 *   const char *payload = "{ \
 *      \"address\": \"xxxx\", \
 *      \"args\": { \
 *         \"method\": \"test_method\", \
 *      } \
 *   }";
 *   uint8_t ret[2048];
 *   uint64_t size = pvm_service_call(service, method, (uint8_t *)payload,
 * strlen(payload), ret);
 *   pvm_assert(2 > 1, "service call success");
 * @endcode
 * @param service[in]: target service name
 * @param method[in]: target service's method name
 * @param payload[in]: pointer to invocation payload
 * @param payload_size[in]: size of payload in bytes
 * @param ret[out]: pointer to buffer for invocation result to write
 * @return size of result in bytes
 * @throw IO(InvalidInput) if service or method is null
 * @throw IO(InvalidData) if service or method is invalid utf-8 string
 * @throw IO(Other) if service call failure
 */
uint64_t pvm_service_call(const char *service, const char *method,
                          const uint8_t *payload, uint64_t payload_size,
                          uint8_t *ret);

#endif
