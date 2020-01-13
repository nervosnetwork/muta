#include <ctype.h>
#include <string.h>

#include "pvm.h"


int pvm_hex2bin(char *s, char *buf)
{
    int i,n = 0;
    for(i = 0; s[i]; i += 2) {
        int c = tolower(s[i]);
        if(c >= 'a' && c <= 'f')
            buf[n] = c - 'a' + 10;
        else buf[n] = c - '0';
        if(s[i + 1] >= 'a' && s[i + 1] <= 'f')
            buf[n] = (buf[n] << 4) | (s[i + 1] - 'a' + 10);
        else buf[n] = (buf[n] << 4) | (s[i + 1] - '0');
        ++n;
    }
    return n;
}

int pvm_bin2hex(uint8_t *bin, uint8_t len, char* out)
{
	uint8_t  i;
	for (i=0; i<len; i++) {
		out[i*2]   = "0123456789abcdef"[bin[i] >> 4];
		out[i*2+1] = "0123456789abcdef"[bin[i] & 0x0F];
	}
	out[len*2] = '\0';
    return 0;
}

int pvm_ret_str(const char *s)
{
    uint8_t *buffer = (uint8_t *)s;
    return pvm_ret(&buffer[0], strlen(buffer));
}

int pvm_ret_u64(uint64_t n)
{
    return pvm_ret((uint8_t*)&n, 8);
}
