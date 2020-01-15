#ifndef PVM_DUKTAPE_EE_HELPER_H_
#define PVM_DUKTAPE_EE_HELPER_H_

#include "./duktape/duktape.h"
#include "pvm.h"

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

static duk_ret_t duk_pvm_load_json_args(duk_context *ctx) {
  duk_push_dynamic_buffer(ctx, 1024);

  void *args = duk_get_buffer(ctx, -1, NULL);
  pvm_load_args(args, NULL);

  duk_buffer_to_string(ctx, -1);
  const char *json = duk_get_string(ctx, -1);
  duk_pop(ctx);

  duk_push_string(ctx, json);
  duk_json_decode(ctx, -1);

  return 1;
}

static duk_ret_t duk_pvm_cycle_limit(duk_context *ctx) {
  uint64_t cycle_limit;

  pvm_cycle_limit(&cycle_limit);
  push_checked_integer(ctx, cycle_limit);

  return 1;
}

static duk_ret_t duk_pvm_get_storage(duk_context *ctx) {
  if (!duk_is_string(ctx, -1)) {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR,
                          "Invalid arguments, key should be string");
    return duk_throw(ctx);
  }

  const char *key = duk_get_string(ctx, -1);
  duk_pop(ctx);

  duk_push_dynamic_buffer(ctx, 1024);
  void *val = duk_get_buffer(ctx, -1, NULL);

  pvm_get_storage((uint8_t *)key, strlen(key), val, NULL);

  duk_buffer_to_string(ctx, -1);
  return 1;
}

static duk_ret_t duk_pvm_set_storage(duk_context *ctx) {
  if (!duk_is_string(ctx, -1) || !duk_is_string(ctx, -2)) {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR,
                          "Invalid arguments, should be string");
    return duk_throw(ctx);
  }

  const char *key = duk_get_string(ctx, -2);
  const char *val = duk_get_string(ctx, -1);
  duk_pop_n(ctx, 2);

  pvm_set_storage((uint8_t *)key, strlen(key), (uint8_t *)val, strlen(val));

  return 0;
}

static duk_ret_t duk_pvm_is_init(duk_context *ctx) {
  uint64_t is_init;
  pvm_is_init(&is_init);
  
  if (0 == is_init) {
      // Push true
      duk_push_true(ctx);
  } else {
      duk_push_false(ctx);
  }

  return 1;
}

static duk_ret_t duk_pvm_contract_call(duk_context *ctx) {
  if (!duk_is_string(ctx, -1) || !duk_is_string(ctx, -2)) {
    duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR,
                          "Invalid arguments, should be string");
    return duk_throw(ctx);
  }

  const char *addr = duk_get_string(ctx, -2);
  const char *call_args = duk_get_string(ctx, -1);
  duk_pop_n(ctx, 2);

  duk_push_dynamic_buffer(ctx, 1024);
  void *ret = duk_get_buffer(ctx, -1, NULL);

  int ret_code = 0;
  if (ret_code != pvm_contract_call((uint8_t *)addr, (uint8_t *)call_args,
                                    strlen(call_args), ret, NULL)) {
    return ret_code;
  }

  duk_buffer_to_string(ctx, -1);
  return 1;
}

void pvm_init(duk_context *ctx) {
  duk_push_object(ctx);

  duk_push_c_function(ctx, duk_pvm_debug, DUK_VARARGS);
  duk_put_prop_string(ctx, -2, "debug");

  duk_push_c_function(ctx, duk_pvm_load_json_args, 0);
  duk_put_prop_string(ctx, -2, "load_json_args");

  duk_push_c_function(ctx, duk_pvm_cycle_limit, 0);
  duk_put_prop_string(ctx, -2, "cycle_limit");

  duk_push_c_function(ctx, duk_pvm_get_storage, 1);
  duk_put_prop_string(ctx, -2, "get_storage");

  duk_push_c_function(ctx, duk_pvm_set_storage, 2);
  duk_put_prop_string(ctx, -2, "set_storage");

  duk_push_c_function(ctx, duk_pvm_contract_call, 2);
  duk_put_prop_string(ctx, -2, "contract_call");

  duk_push_c_function(ctx, duk_pvm_is_init, 0);
  duk_put_prop_string(ctx, -2, "is_init");

  duk_put_global_string(ctx, "PVM");
}

#endif
