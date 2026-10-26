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
