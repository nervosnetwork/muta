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
    // FIXME: overflow?
    char json_args[2048];
    duk_size_t len = 0;
    pvm_load_args((uint8_t *)json_args, &len);

    duk_push_object(ctx);

    duk_push_string(ctx, json_args);
    duk_json_decode(ctx, -1);

    duk_put_global_string(ctx, "ARGS");

    return 0;
}

static duk_ret_t duk_pvm_cycle_limit(duk_context* ctx) {
    uint64_t cycle_limit;

    pvm_cycle_limit(&cycle_limit);
    push_checked_integer(ctx, cycle_limit);

    return 1;
}

static duk_ret_t duk_pvm_get_storage(duk_context *ctx) {
    if (!duk_is_buffer_data(ctx, -1)) {
        duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR, "Invalid arguments");
        return duk_throw(ctx);
    }

    duk_size_t key_size = 0;
    void *key = duk_get_buffer_data(ctx, -1, &key_size);
    duk_pop(ctx);

    duk_size_t val_size = 0;
    duk_push_dynamic_buffer(ctx, 1024);

    void *val = duk_get_buffer(ctx, 0, NULL);
    pvm_get_storage(key, key_size, val, &val_size);

    // Change to ArrayBuffer
    duk_push_buffer_object(ctx, 0, 0, val_size, DUK_BUFOBJ_ARRAYBUFFER);
    duk_swap(ctx, 0, 1);
    duk_pop(ctx);

    return 1;
}

static duk_ret_t duk_pvm_set_storage(duk_context *ctx) {
    if (!duk_is_buffer_data(ctx, -1) || !duk_is_buffer_data(ctx, -2)) {
        duk_push_error_object(ctx, DUK_ERR_EVAL_ERROR, "Invalid arguments");
        return duk_throw(ctx);
    }

    duk_size_t key_size = 0;
    duk_size_t val_size = 0;

    void *key = duk_get_buffer_data(ctx, -2, &key_size);
    void *val = duk_get_buffer_data(ctx, -1, &val_size);
    duk_pop_n(ctx, 2);

    pvm_set_storage(key, key_size, val, val_size);

    return 0;
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

  duk_put_global_string(ctx, "PVM");
}

#endif
