#include <pvm.h>
#include <pvm_extend.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

char** strsplit( const char* s, const char* delim ) {
	void* data;
	char* _s = ( char* )s;
	const char** ptrs;
	unsigned int
		ptrsSize,
		nbWords = 1,
		sLen = strlen( s ),
		delimLen = strlen( delim );

	while ( ( _s = strstr( _s, delim ) ) ) {
		_s += delimLen;
		++nbWords;
	}
	ptrsSize = ( nbWords + 1 ) * sizeof( char* );
	ptrs =
	data = malloc( ptrsSize + sLen + 1 );
	if ( data ) {
		*ptrs =
		_s = strcpy( ( ( char* )data ) + ptrsSize, s );
		if ( nbWords > 1 ) {
			while ( ( _s = strstr( _s, delim ) ) ) {
				*_s = '\0';
				_s += delimLen;
				*++ptrs = _s;
			}
		}
		*++ptrs = NULL;
	}
	return data;
}

#define MAX_COMMAND_LEN 100

int main() {
    char args[MAX_COMMAND_LEN] = {0};
    uint64_t args_len = 0;
    pvm_load_args(args, &args_len);
    pvm_debug(args);
    if (args_len > MAX_COMMAND_LEN) {
        pvm_ret_str("args too long");
        return 1;
    }

    char** arr = strsplit( args, " " );
	if ( !arr ) {
        pvm_ret_str("wrong args, should be like 'set [key] [value]' or 'get [key]'");
        return 1;
	}
    if (strcmp(arr[0], "set") == 0) {
        if (strlen(arr[1]) == 0 || strlen(arr[2]) == 0) {
            pvm_ret_str("wrong args, should be like 'set [key] [value]'");
            return 1;
        }
        pvm_set_storage(arr[1], strlen(arr[1]), arr[2], strlen(arr[2]));
        pvm_debug("set success");
    } else if (strcmp(arr[0], "get") == 0) {
        if (strlen(arr[1]) == 0 ) {
            pvm_ret_str("wrong args, should be like 'get [key]'");
            return 1;
        }
        char val[MAX_COMMAND_LEN] = {0};
        uint64_t val_len = 0;
        pvm_get_storage(arr[1], strlen(arr[1]), val, &val_len);
        pvm_ret(val, val_len);
        pvm_debug("get success");
    } else {
        pvm_ret_str("wrong cmd, should be like 'set [key] [value]' or 'get [key]'");
        return 1;
    }
	free( arr );
	return 0;
}

