# Split a numerical program into files

Use a build identifying as `scarp 0.0.5-alpha.1`; published alpha.4 does not
support imports. Run `cargo run -- --version` in this checkout to verify it.

The shipped example consists of three files:

```text
examples/
  module_demo.srp
  modules/
    linear.srp
    settings.srp
```

Open [`settings.srp`](../../examples/modules/settings.srp). Its public scalar
constants provide a title and a gain. A plain `bind` in a library is private;
adding `pub` allows another file to read it through an alias.

[`linear.srp`](../../examples/modules/linear.srp) imports `"settings.srp"` as
`settings`. This resolves beside `linear.srp`, even when you start Scarp from a
different directory. Its public pure `predict` function constructs weights
inside the function, performs matrix-vector multiplication and scales the
result. Needs/ensures validate the input and result lengths. Keeping numerical
construction in a function avoids runtime module globals.

[`module_demo.srp`](../../examples/module_demo.srp) imports both libraries:

```scarp
import "modules/linear.srp" as linear;
import "modules/settings.srp" as settings;
bind scores = linear.predict([2.0, 1.0]);
print(settings.title);
print(scores);
print(argmax(scores), onehot(len(scores), argmax(scores)));
```

Run and check the entry file from the repository root:

```sh
cargo run --locked --offline -- run examples/module_demo.srp
cargo run --locked --offline -- check examples/module_demo.srp
```

Expected stdout, also stored in `examples/module_demo.stdout`:

```text
Modular inference
[4.0, 1.0]
0 [1.0, 0.0]
```

Stderr is empty. `check` is silent on success. The settings file is shared by
the entry and linear library; the compiler loads its canonical source once.

For an experiment, remove `pub` from `predict` and run the entry again. E0041
points to the call and the private declaration. Restore `pub` to repair it.
Misspelling `predict` produces E0042. Adding a top-level `print` inside a library
produces E0037: move that output into a function or the entry script.

If you add an import from settings back to linear, E0039 identifies the cycle.
Move shared declarations to a third library so dependencies flow in one
direction. Do not use absolute paths or Windows backslashes in import strings;
`/` works on all three supported native platforms.

The complete [module guide](../modules.md) explains visibility, path policy,
source limits, library restrictions and the explicit Rust loader API.
