#include <pvm.h>
#include <pvm_structs.h>

int main() {
  // Test val compare
  pvm_bytes_t val = pvm_bytes_str("test test");
  pvm_bytes_t same = pvm_bytes_str("test test");
  pvm_assert(0 == pvm_bytes_compare(&val, &same), "compare same failed");

  pvm_bytes_t diff = pvm_bytes_str("test diff");
  pvm_assert(0 != pvm_bytes_compare(&val, &diff), "compare diff failed");

  pvm_bytes_t shorter = pvm_bytes_str("test");
  pvm_assert(0 > pvm_bytes_compare(&shorter, &val), "compare bigger failed");
  pvm_assert(0 < pvm_bytes_compare(&val, &shorter), "compare shorter failed");

  // Test str val
  pvm_bytes_t key = pvm_bytes_str("test key");
  pvm_bytes_t str_val = pvm_bytes_str("test val");
  pvm_set(&key, &str_val);

  pvm_bytes_t str_val2 = pvm_bytes_alloc(200);
  pvm_get(&key, &str_val2);
  pvm_assert(0 == pvm_bytes_compare(&str_val, &str_val2), "get set str failed");

  // Test u64 val
  key = pvm_bytes_str("test key2");
  val = pvm_bytes_u64(12345678);
  pvm_set(&key, &val);

  pvm_bytes_t u64_val = pvm_bytes_alloc(8);
  pvm_get(&key, &u64_val);
  pvm_assert(12345678 == pvm_bytes_get_u64(&u64_val), "get set u64 failed");

  // Test val u64 to str
  u64_val = pvm_bytes_u64(12345);
  pvm_bytes_t u64_str = pvm_bytes_u64_to_str(&u64_val);
  pvm_bytes_t expected = pvm_bytes_str("12345");
  pvm_assert(0 == pvm_bytes_compare(&u64_str, &expected), "u64 to str failed");

  // Test val bool
  key = pvm_bytes_str("test key3");
  pvm_set_bool(&key, PVM_TRUE);
  pvm_assert(pvm_get_bool(&key), "get set bool failed");

  // Test realloc
  val = pvm_bytes_alloc(1);
  expected = pvm_bytes_str("hello world");
  pvm_bytes_set_str(&val, "hello world");
  pvm_assert(0 == pvm_bytes_compare(&val, &expected), "realloc str failed");

  val = pvm_bytes_alloc(1);
  expected = pvm_bytes_u64(12345);
  pvm_bytes_set_u64(&val, 12345);
  pvm_assert(0 == pvm_bytes_compare(&val, &expected), "realloc u64 failed");

  // Test append
  pvm_bytes_t dest = pvm_bytes_str("hello");
  pvm_bytes_t src = pvm_bytes_str(" world");
  pvm_bytes_append(&dest, &src);
  expected = pvm_bytes_str("hello world");
  pvm_assert(0 == pvm_bytes_compare(&dest, &expected), "append bytes failed");

  pvm_bytes_append_str(&dest, " fly to the moon");
  expected = pvm_bytes_str("hello world fly to the moon");
  pvm_assert(0 == pvm_bytes_compare(&dest, &expected), "append str failed");

  // Test bytes
  dest = pvm_bytes_alloc(1);
  const char *str = "play gwent";
  pvm_bytes_set_nbytes(&dest, str, strlen(str));
  expected = pvm_bytes_str("play gwent");
  pvm_assert(0 == pvm_bytes_compare(&dest, &expected), "set nbytes failed");

  pvm_bytes_append_nbytes(&dest, " dododo", strlen(" dododo"));
  expected = pvm_bytes_str("play gwent dododo");
  pvm_assert(0 == pvm_bytes_compare(&dest, &expected), "append nbytes failed");

  // Test copy
  src = pvm_bytes_str("hello");
  pvm_bytes_t copy = pvm_bytes_copy(&src);
  pvm_assert(0 == pvm_bytes_compare(&src, &copy), "copy should be same");

  pvm_bytes_set_str(&src, "world");
  pvm_assert(0 != pvm_bytes_compare(&src, &copy),
             "modified src should be different");

  // Test u64
  pvm_u64_t a = pvm_u64_new(1);
  pvm_u64_t b = pvm_u64_new(2);
  pvm_u64_t c = pvm_u64_new(1);
  pvm_assert(-1 == pvm_u64_compare(a, b), "u64 smaller compare failed");
  pvm_assert(1 == pvm_u64_compare(b, a), "u64 bigger compare failed");
  pvm_assert(0 == pvm_u64_compare(a, c), "u64 same compare failed");

  pvm_bytes_t d = pvm_bytes_u64(2);
  pvm_u64_t e = pvm_u64_from_bytes(&d);
  pvm_assert(0 == pvm_u64_compare(e, b), "u64 from bytes failed");

  pvm_bytes_t f = pvm_u64_to_bytes(e);
  pvm_assert(0 == pvm_bytes_compare(&f, &d), "u64 to bytes failed");

  pvm_u64_t g = pvm_u64_add(a, b);
  pvm_assert(0 == pvm_u64_compare(g, pvm_u64_new(3)), "u64 add failed");

  g = pvm_u64_mul(a, b);
  pvm_assert(0 == pvm_u64_compare(g, b), "u64 mul failed");

  g = pvm_u64_sub(pvm_u64_new(2), pvm_u64_new(1));
  pvm_assert(0 == pvm_u64_compare(g, pvm_u64_new(1)), "u64 sub failed");

  // Test array
  pvm_array_t array = pvm_array_new("hello");
  pvm_assert(0 == pvm_array_length(&array), "array length should be 0");

  pvm_bytes_t item = pvm_bytes_str("world");
  pvm_array_push(&array, &item);
  pvm_assert(1 == pvm_array_length(&array), "array length should be 1");

  pvm_bytes_t item2 = pvm_array_get(&array, 0);
  pvm_assert(0 == pvm_bytes_compare(&item, &item2),
             "array item should be same");

  pvm_bytes_t item3 = pvm_array_pop(&array);
  pvm_assert(0 == pvm_bytes_compare(&item, &item3),
             "array item should be same");
  pvm_assert(0 == pvm_array_length(&array), "array length should be 0");

  // Test map
  pvm_bytes_t empty = pvm_bytes_empty();
  pvm_bytes_t empty_key = pvm_bytes_str("empty key");
  pvm_set(&empty_key, &empty);

  pvm_bytes_t empty_val = pvm_bytes_alloc(1);
  pvm_get(&empty_key, &empty_val);
  pvm_assert(pvm_bytes_is_empty(&empty_val), "empty val should be empty");

  pvm_map_t map = pvm_map_new("test map");
  pvm_assert(0 == pvm_map_length(&map), "map length should be 0");

  key = pvm_bytes_str("cdpr");
  item = pvm_bytes_str("2077");
  pvm_map_set(&map, &key, &item);
  pvm_assert(1 == pvm_map_length(&map), "map length should be 1");

  item2 = pvm_map_get(&map, &key);
  pvm_assert(0 == pvm_bytes_compare(&item2, &item), "map item should be same");

  item3 = pvm_map_delete(&map, &key);
  pvm_assert(0 == pvm_bytes_compare(&item3, &item), "map item should be same");
  pvm_assert(0 == pvm_map_length(&map), "map length should be 0");

  return 0;
}
