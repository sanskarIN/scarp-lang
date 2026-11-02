# Exact numeric conversions and Unicode text queries

These are unreleased development features; published 0.0.5-alpha.1 binaries do
not contain them. They are pure built-ins usable in ordinary expressions, typed
functions, defaults and contracts. Module constant initializers still prohibit
all calls. No implicit conversion is added to operators or annotations.

## Checked conversions

| Call | Rule |
| --- | --- |
| `to_float(integer) -> float` | The integer must be exactly representable as binary64 |
| `to_integer(float) -> integer` | The actual float must be integral and lie in [-2^63,2^63) |

`to_float(42)` yields `42.0`; `to_integer(42.0)` yields `42`.
`to_integer(-0.0)` yields `0`. Fractional values are rejected rather than truncated.
Some integers above 2^53 are exactly representable (for example 2^53+2), while
2^53+1 is not. The negative i64 boundary is representable; i64::MAX rounds to 2^63
as binary64 and must be rejected. Conversion checks use a wider integer round trip
to avoid accepting a saturating cast. Float literals already denote binary64
values; these functions do not recover digits rounded during literal parsing.

E0044 points to the argument for precision, fractional or range failures. Argument
types remain strict: `to_float(1.0)` is an E0015 type error. See the
[runnable conversion example](../examples/scalar_conversions.srp).

## Text operations

| Call | Result |
| --- | --- |
| `text_len(text: string) -> integer` | Number of Unicode scalar values |
| `text_bytes(text: string) -> integer` | Number of UTF-8 bytes |
| `text_codepoint(text: string, index: integer) -> integer` | Unicode code point at a scalar index |
| `text_find(text: string, needle: string) -> integer` | First matching scalar index, or -1 |
| `text_contains(text: string, needle: string) -> boolean` | Exact substring match |
| `text_starts_with(text: string, needle: string) -> boolean` | Exact prefix match |
| `text_ends_with(text: string, needle: string) -> boolean` | Exact suffix match |

For `"aé🌍"`, scalar length is 3, byte length is 7, scalar index 2 has code point
127757 and finding `"🌍"` returns 2. Empty needles match at index zero and are
contained/prefixes/suffixes of every string, including empty strings. Negative,
end or oversized scalar indices produce E0045 at the index argument.

Scalars are not user-perceived grapheme clusters. An `e` followed by a combining
acute accent has two scalars and does not equal precomposed `é`. Emoji sequences
can also contain several scalars. No normalization, case folding, character type,
byte-sequence type or locale-dependent behavior is implied. Input strings are not
mutated; these queries return integers/booleans without allocating string results.
See the [Unicode example](../examples/unicode_text.srp).

## Execution work and host APIs

Language dispatch tasks spend the existing step budget. Kernel costs additionally
use the following deterministic allowances, shared by run/check and native host
evaluation. Let n be input UTF-8 byte length and m be needle byte length:

| Operation | Kernel work |
| --- | --- |
| Conversions, byte length | 1 |
| Scalar length/codepoint | max(1,n) |
| Prefix/suffix | max(1,m) |
| Find/contains, nonempty needle | max(1,n*m+n+m), saturating at u64::MAX |
| Find/contains, empty needle | 1 |

Search costs are deliberately conservative, bounding worst-case comparison and
scalar-offset work. They are not timings or exact CPU instruction counts. Charges
precede scans. A negative index is rejected without a scan. Only selected calls
spend work; skipped branches still undergo static argument checking. Numerical
payload quotas are unaffected because these operations allocate no vector/tile.
Existing output capture limits still apply when their results are printed.

The established `numeric::Builtin` registry and `numeric::evaluate` /
`evaluate_with_vector_limit` host APIs expose the same functions, arity, strict
types, domains and kernel costs. Their historical public paths remain compatible;
implementation dispatches scalar/text work to a separate private component.
Hosts exhaustively matching Builtin or DiagnosticKind must handle the new variants.
Old variants/codes, ValueType and execution-option layouts are unchanged.

The current scope excludes string construction, slicing, formatting, normalization
and parsing numbers from strings. Those need explicit allocation and error policies.
See [Proposal 0033](proposals/0033-scalar-text-intrinsics.md).
