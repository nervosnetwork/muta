#ifndef PVM_DUKTAPE_EE_HELPER_H_
#define PVM_DUKTAPE_EE_HELPER_H_

#include "./duktape/duktape.h"
#include "pvm.h"

#define ADDRESS_LEN 50
#define MAX_HASH_LEN 64
// load at most 1KB data
#define MAX_LOAD_SIZE 1024

// Reference: https://github.com/xxuejie/ckb-duktape/blob/master/c/glue.h
duk_double_t dummy_get_now(void) {
  /*
   * Return a fixed time here as a dummy value since CKB does not support
   * fetching current timestamp
   */
  return -11504520000.0;
}

/*
 * Check if v can fit in duk_int_t, if so, push it to duktape stack, otherwise
 * throw an error.
 */
static void push_checked_integer(duk_context *ctx, uint64_t v) {
  if (v == ((uint64_t)((duk_int_t)v))) {
    duk_push_int(ctx, (duk_int_t)v);
  } else {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR, "Integer %lu is overflowed!",
                          v);
    (void)duk_throw(ctx);
  }
}

// ####### PVM #######

static duk_ret_t duk_pvm_debug(duk_context *ctx) {
  duk_push_string(ctx, " ");
  duk_insert(ctx, 0);
  duk_join(ctx, duk_get_top(ctx) - 1);
  pvm_debug(duk_safe_to_string(ctx, -1));

  return 0;
}

static duk_ret_t duk_pvm_load_args(duk_context *ctx) {
  duk_push_fixed_buffer(ctx, MAX_LOAD_SIZE);

  void *args = duk_get_buffer(ctx, 0, NULL);
  pvm_load_args(args, NULL);

  duk_buffer_to_string(ctx, -1);
  duk_push_string(ctx, duk_safe_to_string(ctx, -1));

  return 1;
}

static duk_ret_t duk_pvm_origin(duk_context *ctx) {
  duk_push_fixed_buffer(ctx, ADDRESS_LEN);

  void *args = duk_get_buffer(ctx, 0, NULL);
  pvm_origin(args);

  duk_buffer_to_string(ctx, -1);
  duk_push_string(ctx, duk_safe_to_string(ctx, -1));

  return 1;
}

static duk_ret_t duk_pvm_address(duk_context *ctx) {
  duk_push_fixed_buffer(ctx, ADDRESS_LEN);

  void *args = duk_get_buffer(ctx, 0, NULL);
  pvm_address(args);

  duk_buffer_to_string(ctx, -1);
  duk_push_string(ctx, duk_safe_to_string(ctx, -1));

  return 1;
}

static duk_ret_t duk_pvm_caller(duk_context *ctx) {
  duk_push_fixed_buffer(ctx, ADDRESS_LEN);

  void *args = duk_get_buffer(ctx, 0, NULL);
  pvm_caller(args);

  duk_buffer_to_string(ctx, -1);
  duk_push_string(ctx, duk_safe_to_string(ctx, -1));

  return 1;
}

static duk_ret_t duk_pvm_load_json_args(duk_context *ctx) {
  duk_push_fixed_buffer(ctx, MAX_LOAD_SIZE);

  void *args = duk_get_buffer(ctx, 0, NULL);
  pvm_load_args(args, NULL);

  duk_buffer_to_string(ctx, -1);
  duk_push_string(ctx, duk_safe_to_string(ctx, -1));
  duk_json_decode(ctx, -1);

  return 1;
}

static duk_ret_t duk_pvm_cycle_limit(duk_context *ctx) {
  push_checked_integer(ctx, pvm_cycle_limit());

  return 1;
}

static duk_ret_t duk_pvm_cycle_used(duk_context *ctx) {
  push_checked_integer(ctx, pvm_cycle_used());

  return 1;
}

static duk_ret_t duk_pvm_cycle_price(duk_context *ctx) {
  push_checked_integer(ctx, pvm_cycle_price());

  return 1;
}

static duk_ret_t duk_pvm_block_height(duk_context *ctx) {
  push_checked_integer(ctx, pvm_block_height());

  return 1;
}

// Function duk_pvm_extra inject extra data. If no extra, null
// is returned.
//
// Note: it assumes that injected extra data can be converted
// to String type. Same as pvm_load_args function.
static duk_ret_t duk_pvm_extra(duk_context *ctx) {
  duk_push_fixed_buffer(ctx, MAX_LOAD_SIZE);

  uint64_t extra_sz = 0;
  void *extra = duk_get_buffer(ctx, -1, NULL);
  int no_extra = pvm_extra(extra, &extra_sz);
  if (no_extra) {
    duk_pop(ctx); // Pop previous pushed fixed buffer
    duk_push_null(ctx);
  } else {
    duk_buffer_to_string(ctx, -1);
    duk_push_string(ctx, duk_safe_to_string(ctx, -1));
  }

  return 1;
}

static duk_ret_t duk_pvm_timestamp(duk_context *ctx) {
  push_checked_integer(ctx, pvm_timestamp());

  return 1;
}

static duk_ret_t duk_pvm_emit_event(duk_context *ctx) {
  if (!duk_is_string(ctx, -1)) {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR,
                          "Invalid argument, event message should be string");
    return duk_throw(ctx);
  }

  const char *msg = duk_safe_to_string(ctx, -1);
  duk_pop(ctx);

  if (pvm_emit_event((uint8_t *)msg, strlen(msg))) {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR, "Invalid UTF-8 string");
    return duk_throw(ctx);
  }

  return 0;
}

static duk_ret_t duk_pvm_tx_hash(duk_context *ctx) {
  duk_push_fixed_buffer(ctx, MAX_HASH_LEN);

  void *hash = duk_get_buffer(ctx, 0, NULL);
  if (pvm_tx_hash(hash)) {
    duk_pop(ctx);
    duk_push_null(ctx);
  } else {
    duk_buffer_to_string(ctx, -1);
    duk_push_string(ctx, duk_safe_to_string(ctx, -1));
  }

  return 1;
}

// Function duk_pvm_tx_nonce inject extra data. If no nonce, null
// is returned.
static duk_ret_t duk_pvm_tx_nonce(duk_context *ctx) {
  duk_push_fixed_buffer(ctx, MAX_HASH_LEN);

  void *nonce = duk_get_buffer(ctx, 0, NULL);
  if (pvm_tx_nonce(nonce)) {
    duk_pop(ctx);
    duk_push_null(ctx);
  } else {
    duk_buffer_to_string(ctx, -1);
    duk_push_string(ctx, duk_safe_to_string(ctx, -1));
  }

  return 1;
}

static duk_ret_t duk_pvm_get_storage(duk_context *ctx) {
  if (!duk_is_string(ctx, -1)) {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR,
                          "Invalid arguments, key should be string");
    return duk_throw(ctx);
  }

  const char *key = duk_safe_to_string(ctx, -1);
  duk_pop(ctx);

  duk_push_fixed_buffer(ctx, MAX_LOAD_SIZE);
  void *val = duk_get_buffer(ctx, 0, NULL);

  uint64_t val_size = 0;
  pvm_get_storage((uint8_t *)key, strlen(key), val, &val_size);

  duk_buffer_to_string(ctx, -1);
  duk_push_string(ctx, duk_safe_to_string(ctx, -1));

  return 1;
}

static duk_ret_t duk_pvm_set_storage(duk_context *ctx) {
  if (!duk_is_string(ctx, -1) || !duk_is_string(ctx, -2)) {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR,
                          "Invalid arguments, should be string");
    return duk_throw(ctx);
  }

  const char *key = duk_safe_to_string(ctx, -2);
  const char *val = duk_safe_to_string(ctx, -1);
  duk_pop_n(ctx, 2);

  pvm_set_storage((uint8_t *)key, strlen(key), (uint8_t *)val, strlen(val));

  return 0;
}

static duk_ret_t duk_pvm_is_init(duk_context *ctx) {
  if (pvm_is_init()) {
    duk_push_true(ctx);
  } else {
    duk_push_false(ctx);
  }

  return 1;
}

static duk_ret_t duk_pvm_service_call(duk_context *ctx) {
  if (!duk_is_string(ctx, 0) || !duk_is_string(ctx, 1) ||
      !duk_is_string(ctx, 2)) {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR,
                          "Invalid service_call arguments, should be string");
    return duk_throw(ctx);
  }

  const char *service = duk_safe_to_string(ctx, 0);
  const char *method = duk_safe_to_string(ctx, 1);
  const char *payload = duk_safe_to_string(ctx, 2);
  duk_pop_n(ctx, 3);

  duk_push_fixed_buffer(ctx, MAX_LOAD_SIZE);
  void *ret = duk_get_buffer(ctx, 0, NULL);

  int ret_code = 0;
  if (ret_code != pvm_service_call(service, method, (uint8_t *)payload,
                                   strlen(payload), ret, NULL)) {
    return ret_code;
  }

  duk_buffer_to_string(ctx, -1);
  duk_push_string(ctx, duk_safe_to_string(ctx, -1));

  return 1;
}

static duk_ret_t duk_pvm_contract_call(duk_context *ctx) {
  if (!duk_is_string(ctx, -1) || !duk_is_string(ctx, -2)) {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR,
                          "Invalid arguments, should be string");
    return duk_throw(ctx);
  }

  const char *addr = duk_safe_to_string(ctx, -2);
  const char *call_args = duk_safe_to_string(ctx, -1);
  duk_pop_n(ctx, 2);

  duk_push_fixed_buffer(ctx, MAX_LOAD_SIZE);
  void *ret = duk_get_buffer(ctx, 0, NULL);

  int ret_code = 0;
  if (ret_code != pvm_contract_call((uint8_t *)addr, (uint8_t *)call_args,
                                    strlen(call_args), ret, NULL)) {
    return ret_code;
  }

  duk_buffer_to_string(ctx, -1);
  duk_push_string(ctx, duk_safe_to_string(ctx, -1));

  return 1;
}

void pvm_init(duk_context *ctx) {
  duk_push_object(ctx);

  duk_push_c_function(ctx, duk_pvm_debug, DUK_VARARGS);
  duk_put_prop_string(ctx, -2, "debug");

  duk_push_c_function(ctx, duk_pvm_load_args, 0);
  duk_put_prop_string(ctx, -2, "load_args");

  duk_push_c_function(ctx, duk_pvm_load_json_args, 0);
  duk_put_prop_string(ctx, -2, "load_json_args");

  duk_push_c_function(ctx, duk_pvm_cycle_limit, 0);
  duk_put_prop_string(ctx, -2, "cycle_limit");

  duk_push_c_function(ctx, duk_pvm_cycle_used, 0);
  duk_put_prop_string(ctx, -2, "cycle_used");

  duk_push_c_function(ctx, duk_pvm_cycle_price, 0);
  duk_put_prop_string(ctx, -2, "cycle_price");

  duk_push_c_function(ctx, duk_pvm_origin, 0);
  duk_put_prop_string(ctx, -2, "origin");

  duk_push_c_function(ctx, duk_pvm_caller, 0);
  duk_put_prop_string(ctx, -2, "caller");

  duk_push_c_function(ctx, duk_pvm_address, 0);
  duk_put_prop_string(ctx, -2, "address");

  duk_push_c_function(ctx, duk_pvm_block_height, 0);
  duk_put_prop_string(ctx, -2, "block_height");

  duk_push_c_function(ctx, duk_pvm_extra, 0);
  duk_put_prop_string(ctx, -2, "extra");

  duk_push_c_function(ctx, duk_pvm_timestamp, 0);
  duk_put_prop_string(ctx, -2, "timestamp");

  duk_push_c_function(ctx, duk_pvm_emit_event, 1);
  duk_put_prop_string(ctx, -2, "emit_event");

  duk_push_c_function(ctx, duk_pvm_tx_hash, 0);
  duk_put_prop_string(ctx, -2, "tx_hash");

  duk_push_c_function(ctx, duk_pvm_tx_nonce, 0);
  duk_put_prop_string(ctx, -2, "tx_nonce");

  duk_push_c_function(ctx, duk_pvm_get_storage, 1);
  duk_put_prop_string(ctx, -2, "get_storage");

  duk_push_c_function(ctx, duk_pvm_set_storage, 2);
  duk_put_prop_string(ctx, -2, "set_storage");

  duk_push_c_function(ctx, duk_pvm_contract_call, 2);
  duk_put_prop_string(ctx, -2, "contract_call");

  duk_push_c_function(ctx, duk_pvm_service_call, 3);
  duk_put_prop_string(ctx, -2, "service_call");

  duk_push_c_function(ctx, duk_pvm_is_init, 0);
  duk_put_prop_string(ctx, -2, "is_init");

  duk_put_global_string(ctx, "PVM");
}

#endif
