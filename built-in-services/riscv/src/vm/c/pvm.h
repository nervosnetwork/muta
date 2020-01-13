#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#ifndef _PVM_H
#define _PVM_H

static inline long
__internal_syscall(long n, long _a0, long _a1, long _a2, long _a3, long _a4, long _a5)
{
    register long a0 asm("a0") = _a0;
    register long a1 asm("a1") = _a1;
    register long a2 asm("a2") = _a2;
    register long a3 asm("a3") = _a3;
    register long a4 asm("a4") = _a4;
    register long a5 asm("a5") = _a5;
    register long syscall_id asm("a7") = n;
    asm volatile ("scall": "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a5), "r"(syscall_id));
    return a0;
}

#define syscall(n, a, b, c, d, e, f) \
    __internal_syscall(n, (long)(a), (long)(b), (long)(c), (long)(d), (long)(e), (long)(f))


#define SYSCODE_DEBUG 2000
#define SYSCODE_LOAD_ARGS 2001
#define SYSCODE_RET 2002

#define SYSCODE_CYCLE_LIMIT 3000

#define SYSCODE_GET_STORAGE 4000
#define SYSCODE_SET_STORAGE 4001

// Function pvm_debug accepts a string that contains the text to be written to stdout(It depends on the VM).
// Params:
//   format: same as the standard C function `printf()`
// Return:
//   code: 0(success)
// Example:
//   pvm_debug("Hello World!");
int pvm_debug(const char* s)
{
  return syscall(SYSCODE_DEBUG, s, 0, 0, 0, 0, 0);
}

int pvm_load_args(uint8_t *data, uint64_t *len)
{
    return syscall(SYSCODE_LOAD_ARGS, data, len, 0, 0, 0, 0);
}

// Function ret returns any bytes to host, as the output of the current contract.
// Params:
//   data: a pointer to a buffer in VM memory space denoting where the data we are about to send.
//   size: size of the data buffer
// Return:
//   code: 0(success)
//
// Note: This syscall(s) only allowed to call once. If called it multiple times, the last call will replace the
// previous call.
int pvm_ret(uint8_t *data, size_t size)
{
    return syscall(SYSCODE_RET, data, size, 0, 0, 0, 0);
}

// Function pvm_cycle_limit loads current block cycle_limit.
// Params:
//   cycle_limit: a pointer to a uint64_t in VM memory space denoting where the cycle_limit located at.
// Return:
//   code: 0(success)
int pvm_cycle_limit(uint64_t *cycle_limit)
{
    return syscall(SYSCODE_CYCLE_LIMIT, cycle_limit, 0, 0, 0, 0, 0);
}

int pvm_get_storage(uint8_t *k, uint64_t k_size, uint8_t *v, uint64_t *v_size)
{
    return syscall(SYSCODE_GET_STORAGE, k, k_size, v, v_size, 0, 0);
}

int pvm_set_storage(uint8_t *k, uint64_t k_size, uint8_t *v, uint64_t v_size)
{
    return syscall(SYSCODE_SET_STORAGE, k, k_size, v, v_size, 0, 0);
}

#endif
