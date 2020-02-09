#include <stdint.h>

#include <UsefulBuf.h>
#include <pvm.h>
#include <pvm_extend.h>
#include <pvm_structs.h>

#include "./cJSON.h"

pvm_bytes_t _load_args() {
  uint8_t buf[2048];
  uint64_t args_size;

  pvm_load_args(buf, &args_size);
  return pvm_bytes_nbytes(buf, args_size);
}

pvm_bytes_t _caller() {
  uint8_t buf[50];
  pvm_caller(buf);
  return pvm_bytes_nbytes(buf, 50);
}

pvm_bytes_t _contract_address() {
  uint8_t buf[50];
  pvm_address(buf);
  return pvm_bytes_nbytes(buf, 50);
}

pvm_bytes_t _balance_key(pvm_bytes_t *asset, pvm_bytes_t *account) {
  pvm_bytes_t key = pvm_bytes_str("balance: ");
  pvm_bytes_append(&key, asset);
  pvm_bytes_append_str(&key, ":");
  pvm_bytes_append(&key, account);

  return key;
}

pvm_u64_t _balance(pvm_bytes_t *asset, pvm_bytes_t *account) {
  pvm_bytes_t key = _balance_key(asset, account);
  return pvm_u64_new(pvm_get_u64(&key));
}

void _set_balance(pvm_bytes_t *asset, pvm_bytes_t *account, pvm_u64_t amount) {
  pvm_bytes_t key = _balance_key(asset, account);
  pvm_set_u64(&key, pvm_u64_raw(amount));
}

void deposit(pvm_bytes_t *asset, pvm_u64_t amount) {
  pvm_bytes_t caller = _caller();
  pvm_bytes_t recipient = _contract_address();

  cJSON *args = cJSON_CreateObject();
  pvm_assert(NULL != cJSON_AddStringToObject(args, "method", "transfer_from"),
             "deposit method");
  pvm_assert(NULL != cJSON_AddStringToObject(args, "sender",
                                             pvm_bytes_get_str(&caller)),
             "deposit sender");
  pvm_assert(NULL != cJSON_AddStringToObject(args, "recipient",
                                             pvm_bytes_get_str(&recipient)),
             "deposit recipent");
  pvm_assert(NULL !=
                 cJSON_AddNumberToObject(args, "amount", pvm_u64_raw(amount)),
             "deposit amount");

  const char *json_args = cJSON_Print(args);
  pvm_assert(NULL != json_args, "deposit json args");

  pvm_contract_call(pvm_bytes_raw_ptr(asset), (const uint8_t *)json_args,
                    strlen(json_args), NULL, NULL);

  pvm_u64_t amount_before = _balance(asset, &caller);
  pvm_u64_t amount_after = pvm_u64_sub(amount_before, amount);
  _set_balance(asset, &caller, amount_after);
}

void withdraw(pvm_bytes_t *asset, pvm_u64_t amount) {
  pvm_bytes_t caller = _caller();
  pvm_u64_t amount_before = _balance(asset, &caller);
  pvm_u64_t amount_after = pvm_u64_sub(amount_before, amount);

  cJSON *args = cJSON_CreateObject();
  pvm_assert(NULL != cJSON_AddStringToObject(args, "method", "withdraw"),
             "withdraw method");
  pvm_assert(NULL != cJSON_AddStringToObject(args, "recipient",
                                             pvm_bytes_get_str(&caller)),
             "withdraw caller");
  pvm_assert(NULL !=
                 cJSON_AddNumberToObject(args, "amount", pvm_u64_raw(amount)),
             "withdraw amount");

  const char *json_args = cJSON_Print(args);

  _set_balance(asset, &caller, amount_after);
  pvm_contract_call(pvm_bytes_raw_ptr(asset), (const uint8_t *)json_args,
                    strlen(json_args), NULL, NULL);
}

pvm_u64_t balance_of(pvm_bytes_t *asset, pvm_bytes_t *account) {
  pvm_assert_not_null(asset, "balance of asset null");
  pvm_assert_not_empty(asset, "balance of asset empty");
  pvm_assert_not_null(account, "balance of account null");
  pvm_assert_not_empty(account, "balance of account empty");

  return _balance(asset, account);
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

pvm_u64_t _cjson_get_u64(cJSON *json, const char *item_name) {
  const cJSON *item = NULL;

  item = cJSON_GetObjectItemCaseSensitive(json, item_name);
  pvm_assert(cJSON_IsNumber(item), "item isn't number");

  uint64_t u64;
  memcpy(&u64, &item->valuedouble, sizeof(uint64_t));
  return pvm_u64_new(u64);
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

int main() {
  pvm_bytes_t json = _load_args();
  cJSON *args = cJSON_Parse(pvm_bytes_get_str(&json));
  pvm_assert(args != NULL, "invalid json args");

  const char *method = _cjson_get_str(args, "method");
  pvm_bytes_t asset = _cjson_get_str_bytes(args, "asset");

  if (0 == strcmp(method, "deposit")) {
    pvm_u64_t amount = _cjson_get_u64(args, "amount");

    deposit(&asset, amount);
  }

  else if (0 == strcmp(method, "withdraw")) {
    pvm_u64_t amount = _cjson_get_u64(args, "amount");

    withdraw(&asset, amount);
  }

  else if (0 == strcmp(method, "balance_of")) {
    pvm_bytes_t account = _cjson_get_str_bytes_or_empty(args, "account");
    pvm_u64_t balance = pvm_u64_zero();

    if (!pvm_bytes_is_empty(&account)) {
      balance = balance_of(&asset, &account);
    } else {
      pvm_bytes_t caller = _caller();
      balance = balance_of(&asset, &caller);
    }
    pvm_ret_u64_str(pvm_u64_raw(balance));
  }

  else {
    pvm_assert(1 > 2, "method not found");
  }
}
