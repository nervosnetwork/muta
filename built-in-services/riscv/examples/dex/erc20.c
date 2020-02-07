#include <stdint.h>

#include "./UsefulBuf.h"
#include "./cJSON.h"
#include "./pvm.h"
#include "./pvm_extend.h"
#include "./pvm_structs.h"

pvm_bytes_t _load_args() {
  uint8_t buf[2048];
  uint64_t args_size;

  pvm_load_args(buf, &args_size);
  return pvm_bytes_nbytes(buf, args_size);
}

pvm_bytes_t _balance_key(pvm_bytes_t *account) {
  pvm_bytes_t key = pvm_bytes_str("balance: ");
  pvm_bytes_append(&key, account);
  return key;
}

pvm_bytes_t _caller() {
  uint8_t buf[50];
  pvm_caller(buf);
  return pvm_bytes_nbytes(buf, 50);
}

pvm_u64_t total_supply() {
  pvm_bytes_t supply_key = pvm_bytes_str("supply");
  return pvm_u64_new(pvm_get_u64(&supply_key));
}

void set_balance(pvm_bytes_t *account, pvm_u64_t amount) {
  pvm_assert_not_null(account, "set balance account null");
  pvm_assert_not_empty(account, "set balance account empty");

  pvm_bytes_t key = _balance_key(account);
  pvm_set_u64(&key, pvm_u64_raw(amount));
}

void init(const char *name, const char *symbol, pvm_u64_t supply) {
  pvm_assert(!pvm_is_init(), "init can only be invoked by deploy function");

  pvm_bytes_t name_key = pvm_bytes_str("name");
  pvm_bytes_t symbol_key = pvm_bytes_str("symbol");
  pvm_bytes_t supply_key = pvm_bytes_str("supply");

  pvm_set_str(&name_key, name);
  pvm_set_str(&symbol_key, symbol);
  pvm_set_u64(&supply_key, pvm_u64_raw(supply));

  pvm_bytes_t caller = _caller();
  set_balance(&caller, supply);
}

pvm_u64_t balance_of(pvm_bytes_t *account) {
  pvm_assert_not_null(account, "balance of account null");
  pvm_assert_not_empty(account, "balance of account empty");

  pvm_bytes_t key = _balance_key(account);
  return pvm_u64_new(pvm_get_u64(&key));
}

void _transfer(pvm_bytes_t *sender, pvm_bytes_t *recipient, pvm_u64_t amount) {
  pvm_assert(pvm_u64_raw(amount) > 0, "transfer amount must be positive");

  pvm_u64_t from_balance = balance_of(sender);
  pvm_u64_t to_balance = balance_of(recipient);

  from_balance = pvm_u64_sub(from_balance, amount);
  to_balance = pvm_u64_add(to_balance, amount);

  pvm_set_u64(sender, pvm_u64_raw(from_balance));
  pvm_set_u64(recipient, pvm_u64_raw(to_balance));
}

void transfer(pvm_bytes_t recipient, pvm_u64_t amount) {
  pvm_bytes_t caller = _caller();
  _transfer(&caller, &recipient, amount);
}

pvm_bytes_t _approve_key(pvm_bytes_t *owner, pvm_bytes_t *spender) {
  pvm_bytes_t key = pvm_bytes_alloc(100);
  pvm_bytes_append(&key, owner);
  pvm_bytes_append(&key, spender);

  return key;
}

void _approve(pvm_bytes_t *owner, pvm_bytes_t *spender, pvm_u64_t amount) {
  pvm_bytes_t key = _approve_key(owner, spender);
  pvm_set_u64(&key, pvm_u64_raw(amount));
}

void approve(pvm_bytes_t *spender, pvm_u64_t amount) {
  pvm_bytes_t caller = _caller();
  _approve(&caller, spender, amount);
}

pvm_u64_t allowances(pvm_bytes_t *owner, pvm_bytes_t *spender) {
  pvm_bytes_t key = _approve_key(owner, spender);
  return pvm_u64_new(pvm_get_u64(&key));
}

void transfer_from(pvm_bytes_t *sender, pvm_bytes_t *recipient,
                   pvm_u64_t amount) {
  pvm_bytes_t caller = _caller();
  pvm_u64_t before_allowance = allowances(sender, recipient);
  pvm_u64_t after_allowance = pvm_u64_sub(before_allowance, amount);

  _transfer(sender, recipient, amount);
  _approve(sender, &caller, after_allowance);
}

pvm_bytes_t _cjson_get_str_bytes(cJSON *json, const char *item_name) {
  const cJSON *item = NULL;

  item = cJSON_GetObjectItemCaseSensitive(json, item_name);
  pvm_assert(cJSON_IsString(item), "item isn't string");
  pvm_assert(item->valuestring != NULL, "item is null");

  return pvm_bytes_str(item->valuestring);
}

const char *_cjson_get_str(cJSON *json, const char *item_name) {
  pvm_bytes_t item = _cjson_get_str_bytes(json, item_name);
  return pvm_bytes_get_str(&item);
}

pvm_bytes_t _cjson_get_str_bytes_or_empty(cJSON *json, const char *item_name) {
  const cJSON *item = NULL;

  item = cJSON_GetObjectItemCaseSensitive(json, item_name);
  pvm_assert(cJSON_IsString(item), "item isn't string");

  if (item->valuestring == NULL) {
    return pvm_bytes_empty();
  } else {
    return pvm_bytes_str(item->valuestring);
  }
}

pvm_u64_t _cjson_get_u64(cJSON *json, const char *item_name) {
  const cJSON *item = NULL;

  item = cJSON_GetObjectItemCaseSensitive(json, item_name);
  pvm_assert(cJSON_IsNumber(item), "item isn't number");

  uint64_t u64;
  memcpy(&u64, &item->valuedouble, sizeof(uint64_t));
  return pvm_u64_new(u64);
}

int main() {
  pvm_bytes_t json = _load_args();
  cJSON *args = cJSON_Parse(pvm_bytes_get_str(&json));
  pvm_assert(args != NULL, "invalid json args");

  const char *method = _cjson_get_str(args, "method");
  if (0 == strcmp(method, "init")) {
    const char *name = _cjson_get_str(args, "name");
    const char *symbol = _cjson_get_str(args, "symbol");
    pvm_u64_t supply = _cjson_get_u64(args, "supply");

    init(name, symbol, supply);
  } else if (0 == strcmp(method, "total_supply")) {
    pvm_ret_u64(pvm_u64_raw(total_supply()));
  } else if (0 == strcmp(method, "balance_of")) {
    pvm_bytes_t account = _cjson_get_str_bytes_or_empty(args, "account");

    if (!pvm_bytes_is_empty(&account)) {
      pvm_ret_u64(pvm_u64_raw(balance_of(&account)));
    } else {
      pvm_bytes_t caller = _caller();
      pvm_ret_u64(pvm_u64_raw(balance_of(&caller)));
    }
  } else if (0 == strcmp(method, "transfer")) {
    pvm_bytes_t recipient = _cjson_get_str_bytes(args, "recipient");
    pvm_u64_t amount = _cjson_get_u64(args, "amount");

    transfer(recipient, amount);
  } else if (0 == strcmp(method, "allowances")) {
    pvm_bytes_t owner = _cjson_get_str_bytes(args, "owner");
    pvm_bytes_t spender = _cjson_get_str_bytes(args, "spender");

    allowances(&owner, &spender);
  } else if (0 == strcmp(method, "approve")) {
    pvm_bytes_t spender = _cjson_get_str_bytes(args, "spender");
    pvm_u64_t amount = _cjson_get_u64(args, "amount");

    approve(&spender, amount);
  } else if (0 == strcmp(method, "transfer_from")) {
    pvm_bytes_t sender = _cjson_get_str_bytes(args, "sender");
    pvm_bytes_t recipient = _cjson_get_str_bytes(args, "recipient");
    pvm_u64_t amount = _cjson_get_u64(args, "amount");

    transfer_from(&sender, &recipient, amount);
  } else {
    // Alway false
    pvm_assert(0 > 1, "method not found");
  }

  return 0;
}
