#ifndef _PVM_STRUCTS_H
#define _PVM_STRUCTS_H

#include "UsefulBuf.h"

#define PVM_TRUE 1
#define PVM_FALSE 0
#define PVM_SUCCESS 0;

typedef UsefulOutBuf pvm_bytes_t;

/*
 * Struct pvm_bytes_t
 */
#define pvm_bytes_stack(name, size) UsefulOutBuf_MakeOnStack(name, size);

pvm_bytes_t pvm_bytes_alloc(uint64_t size);
pvm_bytes_t pvm_bytes_empty();
void pvm_bytes_free(pvm_bytes_t *val);

/*
 * Assertion Functions
 */
void pvm_assert_not_null(pvm_bytes_t *bytes, const char *msg);
void pvm_assert_not_empty(pvm_bytes_t *bytes, const char *msg);
void pvm_assert_not_corruption(pvm_bytes_t *bytes, const char *msg);

int pvm_bytes_is_empty(pvm_bytes_t *val);
uint64_t pvm_bytes_len(pvm_bytes_t *val);
int pvm_bytes_compare(pvm_bytes_t *src, pvm_bytes_t *other);
pvm_bytes_t pvm_bytes_copy(pvm_bytes_t *src);

int pvm_bytes_set_u64(pvm_bytes_t *val, uint64_t n);
uint64_t pvm_bytes_get_u64(pvm_bytes_t *val);

int pvm_bytes_set_str(pvm_bytes_t *val, const char *str);
const char *pvm_bytes_get_str(pvm_bytes_t *val);

int pvm_bytes_set_nbytes(pvm_bytes_t *dest, const void *ptr, uint64_t size);
const void *pvm_bytes_raw_ptr(pvm_bytes_t *val);

const pvm_bytes_t pvm_bytes_str(const char *str);
const pvm_bytes_t pvm_bytes_u64(uint64_t n);
const pvm_bytes_t pvm_bytes_u64_to_str(pvm_bytes_t *val);
const pvm_bytes_t pvm_bytes_nbytes(const void *ptr, uint64_t size);

int pvm_bytes_append(pvm_bytes_t *dest, pvm_bytes_t *src);
int pvm_bytes_append_u64(pvm_bytes_t *dest, uint64_t val);
int pvm_bytes_append_str(pvm_bytes_t *dest, const char *src);
int pvm_bytes_append_nbytes(pvm_bytes_t *dest, const void *ptr, uint64_t size);

/*
 * Storage operations
 */
int pvm_set(pvm_bytes_t *key, pvm_bytes_t *val);
uint64_t pvm_get_size(pvm_bytes_t *key);
int pvm_get(pvm_bytes_t *key, pvm_bytes_t *val);
int pvm_set_u64(pvm_bytes_t *key, uint64_t val);
uint64_t pvm_get_u64(pvm_bytes_t *key);
const char *pvm_get_str(pvm_bytes_t *key);
int pvm_set_str(pvm_bytes_t *key, const char *str);
int pvm_set_bool(pvm_bytes_t *key, uint8_t flag);
int pvm_get_bool(pvm_bytes_t *key);

/*
 * Struct pvm_u64_t
 */
typedef struct pvm_u64_t {
  uint64_t val;
} pvm_u64_t;

pvm_u64_t pvm_u64_new(uint64_t n);
pvm_u64_t pvm_u64_zero();
void pvm_u64_dump(pvm_u64_t u64);

uint64_t pvm_u64_raw(pvm_u64_t u64);
pvm_bytes_t pvm_u64_to_bytes(pvm_u64_t u64);
pvm_u64_t pvm_u64_from_bytes(pvm_bytes_t *src);

int pvm_u64_compare(pvm_u64_t left, pvm_u64_t right);

/*
 * Basic operations, if overflow, will abort.
 */
pvm_u64_t pvm_u64_add(pvm_u64_t a, pvm_u64_t b);
pvm_u64_t pvm_u64_sub(pvm_u64_t a, pvm_u64_t b);
pvm_u64_t pvm_u64_mul(pvm_u64_t a, pvm_u64_t b);
pvm_u64_t pvm_u64_div(pvm_u64_t a, pvm_u64_t b);
pvm_u64_t pvm_u64_mod(pvm_u64_t a, pvm_u64_t b);

/*
 * Struct pvm_array_t
 */
typedef struct pvm_array_t {
  pvm_bytes_t name;
} pvm_array_t;

pvm_array_t pvm_array_new(const char *name);
uint64_t pvm_array_length(pvm_array_t *array);
void pvm_array_push(pvm_array_t *array, pvm_bytes_t *item);
pvm_bytes_t pvm_array_get(pvm_array_t *array, uint64_t idx);
pvm_bytes_t pvm_array_pop(pvm_array_t *array);

/*
 * Struct pvm_map_t
 */
typedef struct pvm_map_t {
  pvm_bytes_t name;
} pvm_map_t;

pvm_map_t pvm_map_new(const char *name);
uint64_t pvm_map_length(pvm_map_t *map);
pvm_bytes_t pvm_map_get(pvm_map_t *map, pvm_bytes_t *key);
void pvm_map_set(pvm_map_t *map, pvm_bytes_t *key, pvm_bytes_t *val);
pvm_bytes_t pvm_map_delete(pvm_map_t *map, pvm_bytes_t *key);

#endif
