#include <ctype.h>
#include <string.h>

#include "pvm.h"

/*
 * Function pvm_hex2bin decode given hex string to binary data.
 *
 * Params:
 *   s[in]: string to decode
 *
 *   buf[out]: pointer to buffer for decoded data to write
 *
 * Return:
 *   n: size of decoded data
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

/*
 * Function pvm_bin2hex encode given data to hex string
 *
 * Params:
 *   bin[in]: pointer to data to encode
 *   len[in]: size of data
 *
 *   out[out]: pointer to buffer for encoded string to write
 *
 * Return:
 *   0(success)
 */
int pvm_bin2hex(uint8_t *bin, uint8_t len, char *out) {
  uint8_t i;
  for (i = 0; i < len; i++) {
    out[i * 2] = "0123456789abcdef"[bin[i] >> 4];
    out[i * 2 + 1] = "0123456789abcdef"[bin[i] & 0x0F];
  }
  out[len * 2] = '\0';
  return 0;
}

/*
 * Function pvm_ret_str is a wrapper function to pvm_ret to easily return
 * string.
 *
 * Params:
 *   s[in]: string to return
 *
 * Return:
 *   0(success)
 */
int pvm_ret_str(const char *s) {
  uint8_t *buffer = (uint8_t *)s;
  return pvm_ret(&buffer[0], strlen(buffer));
}

/*
 * Function pvm_ret_u64 is a wrapper function to pvm_ret to easily return
 * uint64_t.
 *
 * Params:
 *   n[in]: given number to return
 *
 * Return:
 *   0(success)
 */
int pvm_ret_u64(uint64_t n) { return pvm_ret((uint8_t *)&n, 8); }
