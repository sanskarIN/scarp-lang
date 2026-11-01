# Proposal 0029 — Explicit source modules

Status: initial implementation contract for development toward 0.0.5.
Published alpha.4 remains unchanged. No new release is authorized.

```scarp
import "libraries/math.srp" as math;
print(math.twice(math.factor));
```

```scarp
// libraries/math.srp
pub bind factor = 2.0;
pub pure funcra twice(value: float) -> float { return value * factor; }
```

Grammar additions at file scope:

```text
import-declaration := import STRING as IDENTIFIER ;
public-declaration := pub ( [pure] funcra ... | bind IDENTIFIER = scalar-literal ; )
qualified-name     := IDENTIFIER . IDENTIFIER
```

Import/public words are contextual; existing identifiers in bindings and calls
remain usable. Imports and public declarations cannot occur inside blocks.
Imported members are always qualified by an explicit alias. Aliases cannot
collide with file-level declarations or another import alias. Nested values use
their existing lexical namespace; they do not replace a qualified module alias.
Imports are private, cannot be re-exported, and are not first-class values.

Only declarations marked `pub` are accessible through an importing alias.
Library files may contain imports, functions, and immutable scalar literal
bindings. Other file-level execution, mutable globals, vector/tile global values
and executable initializers are rejected. Scalars are integer, float, boolean or
string literals, optionally grouped or numerically negated. Library constants are
typed literal values substituted at their uses; they introduce no initialization
side effects or mutable global frames. Normal entry-file statements retain their
existing execution and budget behavior. Runtime module globals are deferred.

Functions have independent module namespaces, forward/mutual local references,
and existing defaults, purity, contracts and argument rules. Module constants
are available in functions, except where shadowed by parameters/local bindings.
An imported function cannot capture the importing file's bindings. Local
functions take precedence over same-named built-ins within their own module.

Loading/lexing/parsing/graph validation/linking finish before execution. Libraries
have no executable initialization; repeated imports and diamond graphs resolve
each canonical source exactly once. Cycles are rejected, including unused ones.
Discovery order is depth-first and source ordered; function identities and
compilation results do not depend on hash-map iteration.

The loader is explicit and independent of lexer/parser/interpreter. The public
library offers filesystem and in-memory implementations of a source-loading
trait, a bounded graph-loading phase, compilation, source-aware validation and
execution. Existing source-string APIs never perform implicit filesystem I/O.
They diagnose imports as requiring the module-loading API.

Filesystem imports use portable `/`-separated relative `.srp` paths. `.` and
`..` are normalized inside the trusted source root; escaping that root is an
error. Absolute paths, backslashes, drive/URI prefixes, NUL/control characters
and empty path components are rejected. Canonicalization also rejects symlink
escapes. CLI run/check trust the entry file's directory and resolve each import
relative to its importing file. Tokens only lexes the requested file. A custom
embedding loader controls its own source trust and stable identities.

Defaults: 128 unique files, 32 import edges on a path (entry depth zero),
1 MiB per source, 8 MiB total source bytes, 1,024 import declarations and 8 MiB
of newly expanded symbol/string text. Loading and linking limits are independent
of runtime work/vector/call/snapshot budgets. Filesystem reads are bounded before
reservation/read completion. A custom loader must respect the supplied byte
allowance; the compiler rechecks its returned data. These are logical source and
expansion limits, not a total host-process memory cap.

Every diagnostic retains its owning file and UTF-8 byte span, including related
definitions and cross-file runtime traces. Existing Span and Diagnostic layouts
remain available; module results own a source map and add a source-aware wrapper.
New codes append E0037–E0043 for module syntax, loading, cycles, loading limits,
visibility, unknown module symbols and path-policy failures. Existing codes are
not renumbered. All semantic failures preserve atomic program output.

No package manager, network imports, background service, website deployment,
user-defined records, tensor framework, native backend or unused IR is added.
