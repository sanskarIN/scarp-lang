/* Generated-unit support: exact-width integers and caller-owned storage. */
#include <stdint.h>
#include <stddef.h>
#include <limits.h>
#if CHAR_BIT != 8 || !defined(INT64_MAX) || !defined(UINT64_MAX)
#error "Scarp requires eight-bit bytes and exact 64-bit integer types"
#endif
#define SCARP_OUTPUT_ERROR 65535u
struct scarp_host {
    void *context;
    int (*write)(void *, const uint8_t *, size_t);
};
/* Additive dual-stream API; existing scarp_host and scarp_run are preserved. */
struct scarp_io {
    const struct scarp_host *stdout_host, *stderr_host;
    void *context;
    int (*flush)(void *, unsigned);
};
struct scarp_workspace {
    int64_t slots[SCARP_SLOT_COUNT];
    int64_t values[SCARP_TEMP_COUNT];
    uint64_t remaining, error_start, error_end;
    size_t prints;
    uint32_t error_code;
    unsigned streams, last_stream;
};
static inline int scarp_fail(struct scarp_workspace *w, uint32_t code,
                            uint64_t start, uint64_t end) {
    w->error_code = code; w->error_start = start; w->error_end = end;
    return 0;
}
static inline int scarp_step(struct scarp_workspace *w, uint64_t start, uint64_t end) {
    if (!w->remaining) return scarp_fail(w, 17u, start, end);
    --w->remaining; return 1;
}
static inline int64_t scarp_from_bits(uint64_t value) {
    return value <= (uint64_t)INT64_MAX ? (int64_t)value
        : -INT64_C(1) - (int64_t)(UINT64_MAX - value);
}
static inline uint64_t scarp_magnitude(int64_t value) {
    return value < 0 ? (uint64_t)(-(value + INT64_C(1))) + UINT64_C(1) : (uint64_t)value;
}
static inline int scarp_binary(struct scarp_workspace *w, unsigned op,
    int64_t a, int64_t b, int64_t *out, uint64_t start, uint64_t end) {
    uint64_t ua, ub, limit, bits;
    switch (op) {
    case 0:
        if ((b > 0 && a > INT64_MAX - b) || (b < 0 && a < INT64_MIN - b)) break;
        *out = a + b; return 1;
    case 1:
        if ((b < 0 && a > INT64_MAX + b) || (b > 0 && a < INT64_MIN + b)) break;
        *out = a - b; return 1;
    case 2:
        ua = scarp_magnitude(a); ub = scarp_magnitude(b);
        limit = ((a < 0) != (b < 0)) ? (UINT64_C(1) << 63) : (uint64_t)INT64_MAX;
        if (ub && ua > limit / ub) break;
        bits = ua * ub;
        *out = scarp_from_bits(((a < 0) != (b < 0)) ? UINT64_C(0) - bits : bits);
        return 1;
    case 3: case 4:
        if (!b) return scarp_fail(w, 8u, start, end);
        if (a == INT64_MIN && b == -INT64_C(1)) break;
        *out = op == 3 ? a / b : a % b; return 1;
    case 5: *out = scarp_from_bits((uint64_t)a & (uint64_t)b); return 1;
    case 6: *out = scarp_from_bits((uint64_t)a | (uint64_t)b); return 1;
    case 7: *out = scarp_from_bits((uint64_t)a ^ (uint64_t)b); return 1;
    case 8: case 9: case 10:
        if (b < 0 || b > 63) return scarp_fail(w, 26u, start, end);
        if (op == 8) bits = (uint64_t)a << (unsigned)b;
        else if (op == 10 || a >= 0) bits = (uint64_t)a >> (unsigned)b;
        else bits = ~((~(uint64_t)a) >> (unsigned)b);
        *out = scarp_from_bits(bits); return 1;
    case 11: *out = a == b; return 1;
    case 12: *out = a != b; return 1;
    case 13: *out = a < b; return 1;
    case 14: *out = a <= b; return 1;
    case 15: *out = a > b; return 1;
    case 16: *out = a >= b; return 1;
    default: return scarp_fail(w, 65534u, start, end);
    }
    return scarp_fail(w, 7u, start, end);
}
static inline int scarp_write(struct scarp_workspace *w, const struct scarp_host *host,
    const uint8_t *bytes, size_t count, uint64_t start, uint64_t end) {
    if (!host || !host->write || host->write(host->context, bytes, count))
        return scarp_fail(w, SCARP_OUTPUT_ERROR, start, end);
    return 1;
}
static inline int scarp_output(struct scarp_workspace *w, const struct scarp_io *io,
    unsigned stream, const uint8_t *bytes, size_t count, uint64_t start, uint64_t end) {
    if (!count) return 1;
    if (!io) return scarp_fail(w, SCARP_OUTPUT_ERROR, start, end);
    if (w->last_stream && w->last_stream != stream + 1u && io->flush &&
        io->flush(io->context, w->last_stream - 1u))
        return scarp_fail(w, SCARP_OUTPUT_ERROR, start, end);
    w->last_stream = stream + 1u;
    return scarp_write(w, stream ? io->stderr_host : io->stdout_host, bytes, count, start, end);
}
static inline int scarp_print(struct scarp_workspace *w, const struct scarp_io *io, unsigned stream,
    int64_t value, int boolean, uint64_t start, uint64_t end) {
    uint8_t buffer[22]; size_t cursor = sizeof buffer;
    uint64_t magnitude;
    static const uint8_t yes[] = {116,114,117,101};
    static const uint8_t no[] = {102,97,108,115,101};
    if (boolean) return scarp_output(w,io,stream,value ? yes : no,value ? sizeof yes : sizeof no,start,end);
    magnitude = scarp_magnitude(value);
    do { buffer[--cursor] = (uint8_t)(48 + magnitude % 10); magnitude /= 10; } while (magnitude);
    if (value < 0) buffer[--cursor] = 45;
    return scarp_output(w,io,stream,buffer + cursor,sizeof buffer - cursor,start,end);
}
