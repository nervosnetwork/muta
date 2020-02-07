#include <stdint.h>
#include <stdio.h>
#include <string.h>

#ifndef USEFULBUF_CONFIG_LITTLE_ENDIAN
#define USEFULBUF_CONFIG_LITTLE_ENDIAN
#endif
#ifndef USEFULBUF_CONFIG_BSWAP
#define USEFULBUF_CONFIG_BSWAP
#endif

#include "./UsefulBuf.h"
#include "./pvm.h"

#define DEFAULT_VAL_SIZE 2048

#define PVM_TRUE 1
#define PVM_FALSE 0
#define PVM_SUCCESS 0;

typedef UsefulOutBuf pvm_bytes_t;

#define pvm_bytes_stack(name, size) UsefulOutBuf_MakeOnStack(name, size);

#define __pvm_input_buf(name, outbuf)                                          \
  UsefulInputBuf name;                                                         \
  UsefulInputBuf_Init(&name, UsefulOutBuf_OutUBuf(outbuf));

void pvm_assert_not_null(pvm_bytes_t *bytes, const char *msg) {
  pvm_assert(!UsefulOutBuf_IsBufferNULL(bytes), msg);
}

void pvm_assert_not_empty(pvm_bytes_t *bytes, const char *msg) {
  pvm_assert(!UsefulBuf_IsEmptyC(UsefulOutBuf_OutUBuf(bytes)), msg);
}

void pvm_assert_not_corruption(pvm_bytes_t *bytes, const char *msg) {
  pvm_assert(!UsefulOutBuf_GetError(bytes), msg);
}

pvm_bytes_t pvm_bytes_alloc(uint64_t size) {
  pvm_bytes_t val;
  uint8_t *buf = malloc(size);

  UsefulBuf ub;
  ub.ptr = buf;
  ub.len = size;

  UsefulOutBuf_Init(&val, ub);

  return val;
}

void pvm_bytes_free(pvm_bytes_t *val) { free(val->UB.ptr); }

int pvm_bytes_is_empty(pvm_bytes_t *val) {
  UsefulBufC buf = UsefulOutBuf_OutUBuf(val);
  if (UsefulBuf_IsEmptyC(buf)) {
    return PVM_TRUE;
  }

  return PVM_FALSE;
}

int pvm_bytes_compare(pvm_bytes_t *src, pvm_bytes_t *other) {
  UsefulBufC src_buf = UsefulOutBuf_OutUBuf(src);
  UsefulBufC other_buf = UsefulOutBuf_OutUBuf(other);

  return UsefulBuf_Compare(src_buf, other_buf);
}

int pvm_bytes_set_u64(pvm_bytes_t *val, uint64_t n) {
  pvm_assert_not_null(val, "set u64 val null");

  if (!UsefulOutBuf_WillItFit(val, sizeof(uint64_t))) {
    pvm_bytes_free(val);
    pvm_bytes_t buf = pvm_bytes_alloc(sizeof(uint64_t));
    val->UB = buf.UB;
  }

  UsefulOutBuf_Reset(val);
  UsefulOutBuf_AppendUint64(val, n);

  return PVM_SUCCESS;
}

uint64_t pvm_bytes_get_u64(pvm_bytes_t *val) {
  pvm_assert_not_null(val, "get u64 val null");
  pvm_assert_not_empty(val, "get u64 val empty");
  pvm_assert_not_corruption(val, "get u64 val corruption");

  __pvm_input_buf(u64, val);
  return UsefulInputBuf_GetUint64(&u64);
}

int pvm_bytes_set_str(pvm_bytes_t *val, const char *str) {
  pvm_assert_not_null(val, "set str val null");

  if (!UsefulOutBuf_WillItFit(val, strlen(str))) {
    pvm_bytes_free(val);
    pvm_bytes_t buf = pvm_bytes_alloc(strlen(str));
    val->UB = buf.UB;
  }

  UsefulOutBuf_Reset(val);
  UsefulOutBuf_AppendString(val, str);

  return PVM_SUCCESS;
}

const char *pvm_bytes_get_str(pvm_bytes_t *val) {
  pvm_assert_not_null(val, "get str val null");
  pvm_assert_not_corruption(val, "get str val corruption");
  if (pvm_bytes_is_empty(val)) {
    return "";
  }

  return val->UB.ptr;
}

int pvm_bytes_set_nbytes(pvm_bytes_t *dest, const void *ptr, uint64_t size) {
  pvm_assert_not_null(dest, "set nbytes dest null");
  pvm_assert(ptr != NULL, "set nbytes ptr null");

  if (!UsefulOutBuf_WillItFit(dest, size)) {
    pvm_bytes_free(dest);
    pvm_bytes_t buf = pvm_bytes_alloc(size);
    dest->UB = buf.UB;
  }

  UsefulOutBuf_Reset(dest);
  UsefulOutBuf_AppendData(dest, ptr, size);

  return PVM_SUCCESS;
}

const void *pvm_bytes_raw_ptr(pvm_bytes_t *val) {
  pvm_assert_not_null(val, "raw ptr null");
  pvm_assert_not_corruption(val, "get str val corruption");

  return val->UB.ptr;
}

const pvm_bytes_t pvm_bytes_str(const char *str) {
  pvm_bytes_t val = pvm_bytes_alloc(strlen(str));
  pvm_bytes_set_str(&val, str);

  return val;
}

const pvm_bytes_t pvm_bytes_u64(uint64_t n) {
  pvm_bytes_t val = pvm_bytes_alloc(sizeof(uint64_t));
  pvm_bytes_set_u64(&val, n);

  return val;
}

const pvm_bytes_t pvm_bytes_nbytes(const void *ptr, uint64_t size) {
  pvm_assert(ptr != NULL, "bytes_n ptr null");

  pvm_bytes_t val = pvm_bytes_alloc(sizeof(size));
  pvm_bytes_set_nbytes(&val, ptr, size);

  return val;
}

const pvm_bytes_t pvm_bytes_u64_to_str(pvm_bytes_t *val) {
  pvm_assert_not_null(val, "u64 to str null");
  pvm_assert_not_empty(val, "u64 to str empty");

  uint64_t n = pvm_bytes_get_u64(val);
  pvm_bytes_t output = pvm_bytes_alloc(24); // Enough to hold uint64_t string
  int size = snprintf(output.UB.ptr, 24, "%lu", n);
  output.data_len = size;

  return output;
}

int pvm_bytes_append(pvm_bytes_t *dest, pvm_bytes_t *src) {
  pvm_assert_not_null(dest, "append dest null");
  pvm_assert_not_null(src, "append src null");
  pvm_assert_not_corruption(src, "append src corruption");

  if (pvm_bytes_is_empty(src)) {
    return PVM_SUCCESS;
  }

  UsefulBufC src_buf = UsefulOutBuf_OutUBuf(src);
  if (!UsefulOutBuf_WillItFit(dest, src_buf.len)) {
    pvm_bytes_t buf = pvm_bytes_alloc(dest->data_len + src_buf.len);
    UsefulOutBuf_AppendUsefulBuf(&buf, UsefulOutBuf_OutUBuf(dest));

    pvm_bytes_free(dest);
    dest->UB = buf.UB;
  }

  UsefulOutBuf_AppendUsefulBuf(dest, src_buf);
  return PVM_SUCCESS;
}

int pvm_bytes_append_u64(pvm_bytes_t *dest, uint64_t val) {
  pvm_bytes_t val_buf = pvm_bytes_u64(val);
  return pvm_bytes_append(dest, &val_buf);
}

int pvm_bytes_append_str(pvm_bytes_t *dest, const char *src) {
  pvm_bytes_t src_buf = pvm_bytes_str(src);
  return pvm_bytes_append(dest, &src_buf);
}

int pvm_bytes_append_nbytes(pvm_bytes_t *dest, const void *ptr, uint64_t size) {
  pvm_bytes_t src_buf = pvm_bytes_nbytes(ptr, size);
  return pvm_bytes_append(dest, &src_buf);
}

int pvm_set(pvm_bytes_t *key, pvm_bytes_t *val) {
  pvm_assert_not_null(key, "set key null");
  pvm_assert_not_empty(key, "set key empty");

  pvm_assert_not_null(val, "set val null");
  if (pvm_bytes_is_empty(val)) {
    return PVM_SUCCESS;
  }

  const void *key_ptr = key->UB.ptr;
  size_t key_len = UsefulOutBuf_GetEndPosition(key);

  const void *val_ptr = val->UB.ptr;
  size_t val_len = UsefulOutBuf_GetEndPosition(val);

  return pvm_set_storage(key_ptr, key_len, val_ptr, val_len);
}

uint64_t pvm_get_size(pvm_bytes_t *key) {
  pvm_assert_not_null(key, "get key null");
  pvm_assert_not_empty(key, "get key empty");

  const void *key_ptr = key->UB.ptr;
  size_t key_len = UsefulOutBuf_GetEndPosition(key);

  return pvm_get_storage_value_size(key_ptr, key_len);
}

int pvm_get(pvm_bytes_t *key, pvm_bytes_t *val) {
  pvm_assert_not_null(key, "get key null");
  pvm_assert_not_empty(key, "get key empty");
  pvm_assert_not_null(val, "get val null");

  const void *key_ptr = key->UB.ptr;
  size_t key_len = UsefulOutBuf_GetEndPosition(key);

  uint64_t val_size = pvm_get_storage_value_size(key_ptr, key_len);
  if (val_size == 0) {
    UsefulOutBuf_Reset(val);
    return PVM_SUCCESS;
  }

  if (!UsefulOutBuf_WillItFit(val, val_size)) {
    pvm_bytes_free(val);
    pvm_bytes_t buf = pvm_bytes_alloc(val_size);
    val->UB = buf.UB;
  }

  return pvm_get_storage(key_ptr, key_len, val->UB.ptr, &val->data_len);
}

uint64_t pvm_get_u64(pvm_bytes_t *key) {
  uint64_t val_size = pvm_get_size(key);
  if (val_size == 0) {
    return 0;
  }

  pvm_assert(val_size == 8, "get u64 wrong size");
  pvm_bytes_stack(u64_to_get, 8);

  pvm_get(key, &u64_to_get);
  return pvm_bytes_get_u64(&u64_to_get);
}

int pvm_set_u64(pvm_bytes_t *key, uint64_t val) {
  pvm_bytes_t v = pvm_bytes_u64(val);
  return pvm_set(key, &v);
}

const char *pvm_get_str(pvm_bytes_t *key) {
  uint64_t val_size = pvm_get_size(key);
  if (val_size == 0) {
    return "";
  }

  pvm_bytes_t val = pvm_bytes_alloc(val_size);
  pvm_get(key, &val);
  return pvm_bytes_get_str(&val);
}

int pvm_set_str(pvm_bytes_t *key, const char *str) {
  pvm_bytes_t v = pvm_bytes_str(str);
  return pvm_set(key, &v);
}

int pvm_set_bool(pvm_bytes_t *key, uint8_t bool) {
  pvm_bytes_stack(bool_to_set, 1);

  if (bool != 0) {
    UsefulOutBuf_AppendByte(&bool_to_set, 1);
  } else {
    UsefulOutBuf_AppendByte(&bool_to_set, 0);
  }

  return pvm_set(key, &bool_to_set);
}

int pvm_get_bool(pvm_bytes_t *key) {
  uint64_t val_size = pvm_get_size(key);
  if (val_size == 0) {
    return PVM_FALSE;
  }

  pvm_assert(val_size == 1, "get bool wrong size");
  pvm_bytes_stack(bool_to_get, 1);
  pvm_get(key, &bool_to_get);

  __pvm_input_buf(input, &bool_to_get);
  if (UsefulInputBuf_GetByte(&input) != 0) {
    return PVM_TRUE;
  }

  return PVM_FALSE;
}

int main() {
  // Test str val
  pvm_bytes_t key = pvm_bytes_str("test key");
  pvm_bytes_t val = pvm_bytes_str("test val");
  pvm_set(&key, &val);

  pvm_debug(pvm_bytes_get_str(&key));
  pvm_debug(pvm_get_str(&key));

  pvm_bytes_t val2 = pvm_bytes_alloc(200);
  pvm_get(&key, &val2);
  pvm_debug(pvm_bytes_get_str(&val2));

  pvm_bytes_free(&key);
  pvm_bytes_free(&val);
  pvm_bytes_free(&val2);

  // Test val compare
  key = pvm_bytes_str("test test");
  val = pvm_bytes_str("test test");
  if (pvm_bytes_compare(&key, &val) == 0) {
    pvm_debug("val matched");
  }
  key = pvm_bytes_str("test");
  if (pvm_bytes_compare(&key, &val) < 0) {
    pvm_debug("key is shorter");
  }

  // Test u64 val
  key = pvm_bytes_str("test key2");
  val = pvm_bytes_u64(12345678);
  pvm_set(&key, &val);

  val2 = pvm_bytes_alloc(8);
  pvm_get(&key, &val2);
  if (pvm_bytes_get_u64(&val2) == 12345678) {
    pvm_debug("get u64");
  }

  pvm_bytes_free(&key);
  pvm_bytes_free(&val);
  pvm_bytes_free(&val2);

  // Test val u64 to str
  pvm_bytes_t u64_str = pvm_bytes_u64_to_str(&val);
  pvm_debug(pvm_bytes_get_str(&u64_str));

  pvm_bytes_free(&u64_str);

  // Test val bool
  key = pvm_bytes_str("test key3");
  pvm_set_bool(&key, PVM_TRUE);
  if (pvm_get_bool(&key)) {
    pvm_debug("get true");
  }

  pvm_bytes_free(&key);

  // Test str realloc
  val = pvm_bytes_alloc(1);
  pvm_bytes_set_str(&val, "hello world");
  pvm_debug(pvm_bytes_get_str(&val));

  pvm_bytes_free(&val);

  // Test u64 realloc
  val = pvm_bytes_alloc(1);
  pvm_bytes_set_u64(&val, 99999);
  if (pvm_bytes_get_u64(&val) == 99999) {
    pvm_debug("realloc u64");
  }

  pvm_bytes_free(&val);

  // Test str append
  val = pvm_bytes_str("hello");
  val2 = pvm_bytes_str(" world");
  pvm_bytes_append(&val, &val2);
  pvm_debug(pvm_bytes_get_str(&val));

  pvm_bytes_append_str(&val, " fly to the moon");
  pvm_debug(pvm_bytes_get_str(&val));

  // Test bytes
  val = pvm_bytes_alloc(1);
  const char *str = "play gwent";
  pvm_bytes_set_nbytes(&val, str, strlen(str));
  pvm_debug(pvm_bytes_raw_ptr(&val));

  pvm_bytes_append_nbytes(&val, "dododo", strlen("dododo"));
  pvm_debug(pvm_bytes_raw_ptr(&val));

  return 0;
}
