#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef USEFULBUF_CONFIG_LITTLE_ENDIAN
#define USEFULBUF_CONFIG_LITTLE_ENDIAN
#endif
#ifndef USEFULBUF_CONFIG_BSWAP
#define USEFULBUF_CONFIG_BSWAP
#endif

#include "pvm.h"
#include "pvm_structs.h"

#define __pvm_input_buf(name, outbuf)                                          \
  UsefulInputBuf name;                                                         \
  UsefulInputBuf_Init(&name, UsefulOutBuf_OutUBuf(outbuf));

void pvm_assert_not_null(pvm_bytes_t *bytes, const char *msg) {
  pvm_assert(bytes != NULL, msg);
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

pvm_bytes_t pvm_bytes_empty() { return pvm_bytes_alloc(0); }

int pvm_bytes_is_empty(pvm_bytes_t *val) {
  UsefulBufC buf = UsefulOutBuf_OutUBuf(val);
  if (UsefulBuf_IsEmptyC(buf)) {
    return PVM_TRUE;
  }

  return PVM_FALSE;
}

uint64_t pvm_bytes_len(pvm_bytes_t *val) {
  pvm_assert_not_null(val, "len val null");
  return val->data_len;
}

int pvm_bytes_compare(pvm_bytes_t *src, pvm_bytes_t *other) {
  pvm_assert_not_null(src, "compare src null");
  pvm_assert_not_null(other, "compare other null");

  UsefulBufC src_buf = UsefulOutBuf_OutUBuf(src);
  UsefulBufC other_buf = UsefulOutBuf_OutUBuf(other);

  return UsefulBuf_Compare(src_buf, other_buf);
}

pvm_bytes_t pvm_bytes_copy(pvm_bytes_t *src) {
  pvm_assert_not_null(src, "copy src null");
  pvm_bytes_t dest = pvm_bytes_alloc(src->data_len);
  UsefulOutBuf_AppendUsefulBuf(&dest, UsefulOutBuf_OutUBuf(src));

  return dest;
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
  pvm_assert_not_corruption(val, "get u64 val corruption");

  if (pvm_bytes_is_empty(val)) {
    return 0;
  }
  if (pvm_bytes_len(val) != 8) {
    return 0;
  }

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

  pvm_bytes_t str = pvm_bytes_alloc(pvm_bytes_len(val) + 1); // +1 for \0
  pvm_bytes_append(&str, val);
  UsefulOutBuf_AppendByte(&str, '\0');

  return pvm_bytes_raw_ptr(&str);
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

  char buf[24];
  uint64_t n = pvm_bytes_get_u64(val);
  int size = snprintf(buf, 24, "%lu", n); // Enough to hold uint64_t string

  pvm_bytes_t output = pvm_bytes_alloc(size);
  pvm_bytes_append_str(&output, buf);

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

  uint64_t val_size = 0;
  pvm_get_storage(key_ptr, key_len, NULL, &val_size);
  return val_size;
}

int pvm_get(pvm_bytes_t *key, pvm_bytes_t *val) {
  pvm_assert_not_null(key, "get key null");
  pvm_assert_not_empty(key, "get key empty");
  pvm_assert_not_null(val, "get val null");

  uint64_t val_size = pvm_get_size(key);
  if (val_size == 0) {
    UsefulOutBuf_Reset(val);
    return PVM_SUCCESS;
  }

  if (!UsefulOutBuf_WillItFit(val, val_size)) {
    pvm_bytes_free(val);
    pvm_bytes_t buf = pvm_bytes_alloc(val_size);
    val->UB = buf.UB;
  }

  const void *key_ptr = key->UB.ptr;
  size_t key_len = UsefulOutBuf_GetEndPosition(key);

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

int pvm_set_bool(pvm_bytes_t *key, uint8_t flag) {
  pvm_bytes_stack(bool_to_set, 1);

  if (flag != 0) {
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

pvm_u64_t pvm_u64_new(uint64_t n) {
  pvm_u64_t u64;
  u64.val = n;

  return u64;
}

pvm_u64_t pvm_u64_zero() { return pvm_u64_new(0); }

void pvm_u64_dump(pvm_u64_t u64) {
    char buf[24];
    snprintf(buf, 24, "%lu", u64);
    pvm_debug(buf);
}

uint64_t pvm_u64_raw(pvm_u64_t u64) { return u64.val; }

pvm_bytes_t pvm_u64_to_bytes(pvm_u64_t u64) { return pvm_bytes_u64(u64.val); }

pvm_u64_t pvm_u64_from_bytes(pvm_bytes_t *src) {
  pvm_assert_not_null(src, "u64 from bytes null");
  if (pvm_bytes_is_empty(src)) {
    return pvm_u64_zero();
  }

  uint64_t n = pvm_bytes_get_u64(src);
  return pvm_u64_new(n);
}

int pvm_u64_compare(pvm_u64_t left, pvm_u64_t right) {
  uint64_t l = left.val;
  uint64_t r = right.val;

  if (l == r)
    return 0;
  else if (l > r)
    return 1;
  else
    return -1;
}

pvm_u64_t pvm_u64_add(pvm_u64_t a, pvm_u64_t b) {
  uint64_t sum;

  pvm_assert(!__builtin_add_overflow(a.val, b.val, &sum), "u64 add overflow");
  return pvm_u64_new(sum);
}

pvm_u64_t pvm_u64_sub(pvm_u64_t a, pvm_u64_t b) {
  uint64_t rem;

  pvm_assert(!__builtin_sub_overflow(a.val, b.val, &rem), "u64 sub overflow");
  return pvm_u64_new(rem);
}

pvm_u64_t pvm_u64_mul(pvm_u64_t a, pvm_u64_t b) {
  uint64_t ret;

  pvm_assert(!__builtin_mul_overflow(a.val, b.val, &ret), "u64 mul overflow");
  return pvm_u64_new(ret);
}

pvm_u64_t pvm_u64_div(pvm_u64_t a, pvm_u64_t b) {
  return pvm_u64_new(a.val / b.val);
}

pvm_u64_t pvm_u64_mod(pvm_u64_t a, pvm_u64_t b) {
  return pvm_u64_new(a.val % b.val);
}

pvm_array_t pvm_array_new(const char *name) {
  pvm_assert(name != NULL, "array name null");
  pvm_assert(strlen(name) != 0, "array name empty");

  pvm_array_t array;
  array.name = pvm_bytes_str(name);

  return array;
}

uint64_t pvm_array_length(pvm_array_t *array) {
  pvm_assert(array != NULL, "array null");
  pvm_assert_not_null(&array->name, "array name null");
  pvm_assert_not_empty(&array->name, "array name empty");

  return pvm_get_u64(&array->name);
}

void pvm_array_push(pvm_array_t *array, pvm_bytes_t *item) {
  pvm_assert(array != NULL, "array null");
  pvm_assert_not_null(item, "item null");
  pvm_assert_not_null(&array->name, "array name null");
  pvm_assert_not_empty(&array->name, "array name empty");

  uint64_t length = pvm_get_u64(&array->name);
  pvm_bytes_t idx_key = pvm_bytes_copy(&array->name);
  pvm_bytes_append_u64(&idx_key, length);

  pvm_set(&idx_key, item);
  pvm_set_u64(&array->name, length + 1);
}

pvm_bytes_t pvm_array_get(pvm_array_t *array, uint64_t idx) {
  pvm_assert(array != NULL, "array null");
  pvm_assert_not_null(&array->name, "array name null");
  pvm_assert_not_empty(&array->name, "array name empty");

  uint64_t length = pvm_get_u64(&array->name);
  pvm_assert(idx < length, "array get out of bound");

  pvm_bytes_t idx_key = pvm_bytes_copy(&array->name);
  pvm_bytes_append_u64(&idx_key, idx);

  uint64_t item_size = pvm_get_size(&idx_key);
  pvm_bytes_t item = pvm_bytes_alloc(item_size);
  pvm_get(&idx_key, &item);

  return item;
}

pvm_bytes_t pvm_array_pop(pvm_array_t *array) {
  pvm_assert(array != NULL, "array null");
  pvm_assert_not_null(&array->name, "array name null");
  pvm_assert_not_empty(&array->name, "array name empty");

  uint64_t length = pvm_get_u64(&array->name);
  pvm_bytes_t last_idx = pvm_bytes_copy(&array->name);
  pvm_bytes_append_u64(&last_idx, length - 1);

  uint64_t item_size = pvm_get_size(&last_idx);
  pvm_bytes_t item = pvm_bytes_alloc(item_size);
  pvm_get(&last_idx, &item);

  pvm_set_u64(&array->name, length - 1);

  return item;
}

pvm_map_t pvm_map_new(const char *name) {
  pvm_assert(name != NULL, "map name null");
  pvm_assert(strlen(name) != 0, "map name empty");

  pvm_map_t map;
  map.name = pvm_bytes_str(name);

  return map;
}

uint64_t pvm_map_length(pvm_map_t *map) {
  pvm_assert(map != NULL, "map null");
  pvm_assert_not_null(&map->name, "map name null");
  pvm_assert_not_empty(&map->name, "map name empty");

  return pvm_get_u64(&map->name);
}

pvm_bytes_t pvm_map_get(pvm_map_t *map, pvm_bytes_t *key) {
  pvm_assert(map != NULL, "map null");
  pvm_assert_not_null(key, "map key null");
  pvm_assert_not_null(&map->name, "map name null");
  pvm_assert_not_empty(&map->name, "map name empty");

  pvm_bytes_t map_key = pvm_bytes_copy(&map->name);
  pvm_bytes_append(&map_key, key);

  uint64_t val_size = pvm_get_size(&map_key);
  pvm_bytes_t val = pvm_bytes_alloc(val_size);
  pvm_get(&map_key, &val);

  return val;
}

void pvm_map_set(pvm_map_t *map, pvm_bytes_t *key, pvm_bytes_t *val) {
  pvm_assert(map != NULL, "map null");
  pvm_assert_not_null(&map->name, "map name null");
  pvm_assert_not_empty(&map->name, "map name empty");

  pvm_assert_not_null(key, "map key null");
  pvm_assert_not_null(val, "map val null");

  pvm_bytes_t map_key = pvm_bytes_copy(&map->name);
  pvm_bytes_append(&map_key, key);
  pvm_set(&map_key, val);

  uint64_t length = pvm_get_u64(&map->name);
  pvm_set_u64(&map->name, length + 1);
}

pvm_bytes_t pvm_map_delete(pvm_map_t *map, pvm_bytes_t *key) {
  pvm_assert(map != NULL, "map null");
  pvm_assert_not_null(key, "map key null");
  pvm_assert_not_null(&map->name, "map name null");
  pvm_assert_not_empty(&map->name, "map name empty");

  pvm_bytes_t map_key = pvm_bytes_copy(&map->name);
  pvm_bytes_append(&map_key, key);

  uint64_t val_size = pvm_get_size(&map_key);
  pvm_bytes_t val = pvm_bytes_alloc(val_size);
  pvm_get(&map_key, &val);

  uint64_t length = pvm_get_u64(&map->name);
  pvm_set_u64(&map->name, length - 1);

  pvm_bytes_t empty = pvm_bytes_empty();
  pvm_set(&map_key, &empty);

  return val;
}
