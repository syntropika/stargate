#ifndef STARGATE_H
#define STARGATE_H
#include <stdint.h>
uint32_t stargate_abi_version(void);
/* UTF-8 NUL-terminated input; returned JSON is owned by the caller. */
char *stargate_call(uint64_t handle, const char *operation, const char *input);
void stargate_free(char *output);
uint32_t stargate_destroy(uint64_t handle);
#endif
