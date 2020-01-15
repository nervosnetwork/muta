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

void pvm_init(duk_context *ctx) {
  duk_push_object(ctx);

  duk_push_c_function(ctx, duk_pvm_debug, DUK_VARARGS);
  duk_put_prop_string(ctx, -2, "debug");

  duk_push_c_function(ctx, duk_pvm_load_json_args, 0);
  duk_put_prop_string(ctx, -2, "load_json_args");

  duk_put_global_string(ctx, "PVM");
}

#endif
