
/* Hosted entry point appended by the native build driver. */
#include <stdio.h>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif
static int scarp_host_output(void *context, const uint8_t *bytes, size_t count) {
    (void)context;
    return fwrite(bytes, 1, count, stdout) == count ? 0 : 1;
}
int main(void) {
    static struct scarp_workspace workspace;
    struct scarp_host host;
    uint32_t code;
#ifdef _WIN32
    if (_setmode(_fileno(stdout), _O_BINARY) == -1 ||
        _setmode(_fileno(stderr), _O_BINARY) == -1) {
        fputs("error: could not set binary output mode\n", stderr);
        return 1;
    }
#endif
    host.context = 0;
    host.write = scarp_host_output;
    code = scarp_run(&workspace, &host, SCARP_MAX_STEPS, SCARP_MAX_PRINTS);
    if (code) {
        if (code == SCARP_OUTPUT_ERROR) fputs("error: could not write output\n", stderr);
        else fprintf(stderr, "error[E%04u] at source bytes %llu..%llu\n",
            (unsigned)code, (unsigned long long)workspace.error_start,
            (unsigned long long)workspace.error_end);
        return 1;
    }
    if (fflush(stdout)) {
        fputs("error: could not flush output\n", stderr);
        return 1;
    }
    return 0;
}
