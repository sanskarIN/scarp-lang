/* Hosted adapter for a generated program.c; the generated unit has no stdio. */
#include "program.c"
#include <stdio.h>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif
static int output(void *context, const uint8_t *bytes, size_t count) {
    (void)context;
    return fwrite(bytes, 1, count, stdout) == count ? 0 : 1;
}
int main(void) {
    static struct scarp_workspace workspace;
    struct scarp_host host;
#ifdef _WIN32
    if (_setmode(_fileno(stdout), _O_BINARY) == -1) return 2;
#endif
    host.context = 0;
    host.write = output;
    if (scarp_run(&workspace, &host, UINT64_C(1000000), 100000)) return 1;
    return fflush(stdout) ? 1 : 0;
}
