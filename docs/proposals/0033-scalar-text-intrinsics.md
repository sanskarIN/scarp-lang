# Proposal 0033 - Exact conversions and bounded Unicode text queries

Status: unreleased development, dependency-free Rust 2021/minimum 1.74.

Add pure typed `to_float(integer)->float` and `to_integer(float)->integer`.
Conversions never round or truncate: integer-to-float must be exactly representable;
float-to-integer must be integral and in [-2^63,2^63). The upper float boundary is
exclusive because i64::MAX rounds to 2^63 as binary64. Compare integer round trips
through i128, not saturating i64 casts. Negative zero converts to integer zero.
E0044 identifies the argument and explains loss/range/fraction failures. Existing
mixed-kind arithmetic stays invalid; module constant calls remain prohibited.

Add text_len, text_bytes, text_codepoint, text_find, text_contains,
text_starts_with and text_ends_with. Inputs are immutable UTF-8 strings. Length and
indices count Unicode scalar values, not bytes/graphemes; text_bytes counts UTF-8.
text_codepoint returns the scalar's integer code point and uses E0045 for negative
or out-of-range indices. Find returns a scalar index or -1; an empty needle finds
index zero. No normalization, case folding, mutation, string-result allocation or
new character type is implied. Prefix/suffix/contains use exact UTF-8 content.

Preserve the existing public numeric::Builtin registry and host evaluation APIs,
extending its variants/signatures while dispatching scalar/text implementation
into a separate private module. All paths share type/arity checks, argument spans
and execution budgets. Numerical vector/tile payload accounting is unchanged.

Kernel work: conversions/byte length cost one; scalar length/codepoint charge
max(1,UTF-8 length); prefix/suffix charge max(1,needle bytes). Find/contains charge a
conservative bound max(1,haystack bytes * needle bytes + haystack bytes + needle
bytes), with saturating u64 arithmetic; empty needles cost one. This intentionally
bounds even worst-case substring comparisons without promising CPU-instruction
counts. Charge before scans; selected calls only. Host APIs enforce identical
costs. Domain/type diagnostics precede scans where no text traversal is necessary.

Test actual representability near 2^53/2^63, fractional and signed-zero cases,
host/language/type/arity parity, purity/defaults/imports, Unicode/combining/emoji,
empty and invalid indices, exact budgets, no vector allocation, deterministic
generated reference cases, exact CLI diagnostic fixtures and runnable examples.
Public Builtin/DiagnosticKind exhaustive host matches need new arms, documented
alongside stable old variants/codes. No external capabilities or release changes.
