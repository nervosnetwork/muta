#include <ctype.h>
#include <string.h>

#include "pvm.h"

/**
 * @brief convert hex string to bytes
 *
 * Function pvm_hex2bin decode given hex string to binary data.
 *
 * @param s[in] hex string to decode
 * @param buf[out]: pointer to buffer for decoded data to write
 * @return size of decoded data
 */
int pvm_hex2bin(char *s, char *buf) {
  int i, n = 0;
  for (i = 0; s[i]; i += 2) {
    int c = tolower(s[i]);
    if (c >= 'a' && c <= 'f')
      buf[n] = c - 'a' + 10;
    else
      buf[n] = c - '0';
    if (s[i + 1] >= 'a' && s[i + 1] <= 'f')
      buf[n] = (buf[n] << 4) | (s[i + 1] - 'a' + 10);
    else
      buf[n] = (buf[n] << 4) | (s[i + 1] - '0');
    ++n;
  }
  return n;
}

/**
 * @brief encode bytes to hex string
 *
 * Function pvm_bin2hex encode given data to hex string
 *
 * @param bin[in]: pointer to data to encode
 * @param len[in]: size of data
 * @param out[out]: pointer to buffer for encoded string to write
 * @return Void
 */
void pvm_bin2hex(uint8_t *bin, uint8_t len, char *out) {
  uint8_t i;
  for (i = 0; i < len; i++) {
    out[i * 2] = "0123456789abcdef"[bin[i] >> 4];
    out[i * 2 + 1] = "0123456789abcdef"[bin[i] & 0x0F];
  }
  out[len * 2] = '\0';
}

/**
 * @brief wapper to pvm_ret to return string
 *
 * Function pvm_ret_str is a wrapper function to pvm_ret to easily return
 * string.
 *
 * @param s[in]: string to return
 * @return Void
 */
void pvm_ret_str(const char *s) {
  uint8_t *buffer = (uint8_t *)s;
  pvm_ret((uint8_t *)s, strlen(buffer));
}

/**
 * @brief wrapper to pvm_ret to return uint64_t
 *
 * Function pvm_ret_u64 is a wrapper function to pvm_ret to easily return
 * uint64_t.
 *
 * @param n[in]: given number to return
 * @return Void
 */
void pvm_ret_u64(uint64_t n) { pvm_ret((uint8_t *)&n, 8); }

/**
 * @brief wrapper to pvm_ret to return uint64_t in string
 *
 * Function pvm_ret_u64_str is a wrapper function to pvm_ret to easily return
 * uint64_t in string.
 *
 * @param n[in]: given number to return
 * @return Void
 */
void pvm_ret_u64_str(uint64_t n) {
  char buf[24];

  size_t size = snprintf(buf, 24, "%lu", n);
  pvm_ret((uint8_t *)buf, size);
}
