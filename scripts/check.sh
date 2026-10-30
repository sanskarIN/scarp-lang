#!/usr/bin/env sh
set -eu

case "${1-}" in
    "") ;;
    --offline) export CARGO_NET_OFFLINE=true ;;
    *) echo "Usage: sh scripts/check.sh [--offline]" >&2; exit 2 ;;
esac
if [ "$#" -gt 1 ]; then
    echo "Usage: sh scripts/check.sh [--offline]" >&2
    exit 2
fi

cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export CARGO_INCREMENTAL=0
export RUSTDOCFLAGS="${RUSTDOCFLAGS-} -D warnings"

cargo fmt --all -- --check
rustfmt --edition 2021 --check scripts/benchmarks/compare.rs
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --release
cargo build --locked
cargo doc --locked --no-deps
cargo run --locked -- run examples/hello.srp
cargo run --locked -- tokens examples/hello.srp
cargo run --locked -- run examples/arithmetic.srp
cargo run --locked -- run examples/bindings.srp
cargo run --locked -- run examples/mutable.srp
cargo run --locked -- run examples/scopes.srp
cargo run --locked -- check examples/scopes.srp
cargo run --locked -- run examples/string_bindings.srp
cargo run --locked -- check examples/string_bindings.srp
cargo run --locked -- run examples/booleans.srp
cargo run --locked -- check examples/booleans.srp
cargo run --locked -- run examples/decisions.srp
cargo run --locked -- check examples/decisions.srp
cargo run --locked -- run examples/loops.srp
cargo run --locked -- check examples/loops.srp
cargo run --locked -- run examples/repeat.srp
cargo run --locked -- check examples/repeat.srp

cargo run --locked -- run examples/escapes.srp
cargo run --locked -- check examples/escapes.srp
cargo run --locked -- run examples/control_escapes.srp
cargo run --locked -- check examples/control_escapes.srp
for example in vectors vector_probabilities coefficient_fit; do
    cargo run --locked -- run "examples/$example.srp"
    cargo run --locked -- check "examples/$example.srp"
done
