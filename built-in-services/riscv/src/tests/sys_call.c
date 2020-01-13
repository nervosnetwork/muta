// A c file contains all system calls supported.
// Used as test and example.
#include <pvm.h>
#include <pvm_extend.h>
#include <string.h>
#include <stdio.h>

int is13(char* data) {
    if (strcmp(data, "13") == 0) {
        return 0;
    }
    if (strcmp(data, "0xd") == 0) {
        return 0;
    }
    if (strcmp(data, "0o15") == 0) {
        return 0;
    }
    if (strcmp(data, "0b1101") == 0) {
        return 0;
    }
    return 1;
}

int main() {
    char debug[1000] = {0};

    // get cycle_limit
    uint64_t cycle_limit = 0;
    pvm_cycle_limit(&cycle_limit);
    memset(debug, 0, sizeof debug);
    sprintf(debug, "cycle limit is %d", cycle_limit);
    pvm_debug(debug);

    // set and get
    char* key = "key";
    char* val = "val";
    pvm_set_storage(key, strlen(key), val, strlen(val));
    char ret_val[5] = {0};
    uint64_t val_size = 0;
    pvm_get_storage(key, strlen(key), ret_val, &val_size);
    memset(debug, 0, sizeof debug);
    sprintf(debug, "return val: %s, val size: %d", ret_val, val_size);
    pvm_debug(debug);


    // load args
    char args[100];
    uint64_t len = 0;
    pvm_load_args(args, &len);

    // check is 13
    char ret[100] = {0};
    if (is13(args) == 0) {
        sprintf(ret, "'%s' is 13", args);
    } else {
        sprintf(ret, "'%s' is not 13", args);
    }
    pvm_ret_str(ret);

    return 0;
}