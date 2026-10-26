
/* Hosted entry point appended by the native build driver. */
#include <stdio.h>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif
static int scarp_host_output(void *context, const uint8_t *bytes, size_t count) {
    return fwrite(bytes, 1, count, (FILE *)context) == count ? 0 : 1;
}
static int scarp_host_flush(void *context, unsigned stream) {
    (void)context;
    return fflush(stream ? stderr : stdout) ? 1 : 0;
}
int main(void) {
    static struct scarp_workspace workspace;
    struct scarp_host output, errors;
    struct scarp_io io;
    uint32_t code;
#ifdef _WIN32
    if (_setmode(_fileno(stdout), _O_BINARY) == -1 ||
        _setmode(_fileno(stderr), _O_BINARY) == -1) {
        fputs("error: could not set binary output mode\n", stderr);
        return 1;
    }
#endif
    output.context = stdout; output.write = scarp_host_output;
    errors.context = stderr; errors.write = scarp_host_output;
    io.stdout_host = &output; io.stderr_host = &errors;
    io.context = 0; io.flush = scarp_host_flush;
    code = scarp_run_io(&workspace, &io, SCARP_MAX_STEPS, SCARP_MAX_PRINTS);
    if (code) {
        if (code == SCARP_OUTPUT_ERROR) fputs("error: could not write output\n", stderr);
        else fprintf(stderr, "error[E%04u] at source bytes %llu..%llu\n",
            (unsigned)code, (unsigned long long)workspace.error_start,
            (unsigned long long)workspace.error_end);
        return 1;
    }
    {
    int output_error = fflush(stdout), error_error = fflush(stderr);
    if (output_error || error_error) {
        fputs("error: could not flush output\n", stderr);
        return 1;
    }
    }
    return 0;
}
