# Your first numerical program

This tutorial uses the alpha.2 numerical implementation. Build the current source
with `cargo build --locked --offline`, then verify `cargo run -- --version`.
An older alpha.1 executable accepts floats but does not implement vectors.
For current downloads, use the
[latest prerelease](https://github.com/scarp-lang/scarp/releases/tag/v0.0.5-alpha.1),
which includes these kernels and the later module system.

## Construct data and read an element

```scarp
bind samples = [1.0, 2.0, 3.0];
print(len(samples), samples[0]);
print(sum(samples), mean(samples));
```

Exact stdout is `3 1.0` then `6.0 2.0`, each followed by LF. Elements must be
floats: use `1.0` rather than `1`. There is no implicit integer conversion.
`samples[0]` is the first element; index 3 is outside this vector.

## Replace a value without changing its copies

```scarp
var weights = fill(0.5, 3);
bind snapshot = weights;
weights = scale(weights, 2.0);
print(snapshot);
print(weights);
```

This prints `[0.5, 0.5, 0.5]` and `[1.0, 1.0, 1.0]`. Fill takes a float value
first, then an integer count. A mutable binding permits replacing the whole
vector. Elements remain immutable, and copies/pending prints keep their original
values. Use add/scale/clamp to create a result rather than assigning an element.

## Put numerical work in a typed function

```scarp
pure funcra squared_error(predicted: vector, expected: vector) -> float
    needs len(predicted) > 0;
    needs len(predicted) == len(expected);
{
    return mse(predicted, expected);
}

bind inputs = [1.0, 2.0, 3.0];
bind targets = [2.0, 4.0, 6.0];
bind predictions = scale(inputs, 2.0);
print(dot(inputs, inputs));
print(squared_error(expected: targets, predicted: predictions));
```

Exact stdout is `14.0` then `0.0`. Named arguments work for user functions;
native kernels require positional arguments. The contracts make this function's
nonempty/equal-length requirements explicit. A failing contract or kernel writes
no pending program output to either stream.

## Check probabilities with a tolerance

```scarp
bind probabilities = softmax([1000.0, 1001.0, 1002.0]);
bind total = sum(probabilities);
print(total > 0.999999999999 and total < 1.000000000001);
print(probabilities[0] < probabilities[1] and probabilities[1] < probabilities[2]);
```

Both lines print true. Softmax shifts by the maximum to avoid an overflowing
exponential. Nontrivial exponential results can vary slightly between platforms,
so use numerical tolerances rather than asserting their exact printed digits.

## Run a complete calculation with limits

[Coefficient fitting](../../examples/coefficient_fit.srp) combines vectors,
an explicit squared-error gradient, 40 iterations and a typed loss function.
Both fixture lines are true. Its derivative is written in the program.

```sh
cargo run -- run examples/coefficient_fit.srp
cargo run -- check --max-steps 200000 --max-capture-bytes 1048576 examples/coefficient_fit.srp
```

Check executes the selected calculations and enforces the same quotas while
remaining silent. Work limits also bound cumulative vector payload allocation;
they are not a total process-memory cap. See [budgets](../budgets.md),
[the vector reference](../vectors.md) and [numerical errors](../reference/numerical-errors.md).
