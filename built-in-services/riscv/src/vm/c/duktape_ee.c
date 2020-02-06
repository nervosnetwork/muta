/*
 * ECMAScript execution environment
 */
#include "duktape/duktape.h"
#include "duktape_ee_helper.h"
#include "pvm.h"

#define EE_ERR_ARGC_NUM 1
#define EE_ERR_INIT_CTX 2
#define EE_ERR_COMPILE_CODE 3

int main(int argc, char *argv[]) {
  // Arguments should be exactly 'main' and 'js code'
  if (2 != argc) {
    return EE_ERR_ARGC_NUM;
  }

  duk_context *ctx = duk_create_heap_default();
  if (!ctx) {
    return EE_ERR_INIT_CTX;
  }

  pvm_init(ctx);

  // Compile code
  char *code = argv[1];

  // Insert Main() invoke
  duk_push_string(ctx, "\n");
  duk_push_string(ctx, code);
  duk_push_string(ctx, "main();");
  duk_join(ctx, 2);

  const char *main_code = duk_get_string(ctx, -1);

  if (0 != duk_pcompile_string(ctx, DUK_COMPILE_EVAL, main_code)) {
    const char *err_msg = duk_get_string(ctx, -1);
    pvm_ret((uint8_t *)err_msg, strlen(err_msg));
  }

  // Call our funtion
  duk_int_t rc = duk_pcall(ctx, 0);
  if (DUK_EXEC_SUCCESS == rc) {
    const char *ret = duk_get_string(ctx, -1);
    pvm_ret((uint8_t *)ret, strlen(ret));
  } else {
    const char *err_msg = duk_safe_to_string(ctx, -1);
    pvm_ret((uint8_t *)err_msg, strlen(err_msg));
  }

  duk_pop(ctx);
  duk_destroy_heap(ctx);

  return rc;
}
