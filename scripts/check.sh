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
cargo run --locked -- exec examples/hello.srp
cargo run --locked -- inspect tokens examples/hello.srp
cargo run --locked -- exec examples/arithmetic.srp
cargo run --locked -- exec examples/bindings.srp
cargo run --locked -- exec examples/mutable.srp
cargo run --locked -- exec examples/scopes.srp
cargo run --locked -- verify examples/scopes.srp
cargo run --locked -- exec examples/string_bindings.srp
cargo run --locked -- verify examples/string_bindings.srp
cargo run --locked -- exec examples/booleans.srp
cargo run --locked -- verify examples/booleans.srp
cargo run --locked -- exec examples/decisions.srp
cargo run --locked -- verify examples/decisions.srp
cargo run --locked -- exec examples/loops.srp
cargo run --locked -- verify examples/loops.srp
cargo run --locked -- exec examples/decimals.srp
cargo run --locked -- verify examples/decimals.srp
cargo run --locked -- exec examples/numerics.srp
cargo run --locked -- verify examples/numerics.srp
cargo run --locked -- exec examples/linear_fit.srp
cargo run --locked -- verify examples/linear_fit.srp
cargo run --locked -- exec examples/bit_fields.srp
