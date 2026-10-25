# Integer bit operations

Status: implemented on `scarp/integer-bit-operations`, dependent on the numerical
development stack. Unreleased.

Hexadecimal `0xFF`/`0XFF` and binary `0b1010`/`0B1010` are signed integer
magnitudes. They use the same i64 range as decimal literals; a directly negated
magnitude of 2^63 represents the minimum integer. High-bit positive literals and
digit separators are rejected. Use `~0` or shifts to construct signed bit patterns.
This extension does not introduce unsigned types or change decimal arithmetic.

From strongest to weakest, the added precedence is: unary `~`, product including
integer `%`, sum, shifts `<<`/`>>`/`>>>`, `&`, `^`, `|`, comparison, then existing
boolean operators. Bit operations require integers, never booleans or floats.
One unary `-` or `~` precedes a primary; group combinations explicitly.

`&`, `|`, `^`, and `~` operate on 64-bit two's-complement patterns. `<<` discards
high bits and inserts low zeros; it is a bit shift, not checked multiplication.
`>>` extends the sign, while `>>>` inserts zeros at the high end. A zero shift
preserves the original pattern. Counts outside 0..63 produce `E0026` at the
operator, including compound assignments. `%` truncates with the dividend's
sign. Zero divisors use E0008; minimum-integer remainder by -1 uses E0007.

Mutable integers support `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`, and `>>>=`.
Assignments preserve copies and pending prints. Arithmetic/bitwise type checks
apply even in skipped paths, while unreachable shift/division failures are not
evaluated. Source and runtime failures retain empty output. `and`/`or` remain
the boolean operators; `&&`/`||` are not aliases.

Examples pack/unpack nibbles, inspect flags, and exercise all 64 bits. Tests cover
precedence, token longest match, signed bounds, shift counts, compounds, generated
packing cases, long chains, and remainder-based loops. The bit-field example is
also run from extracted native packages.
