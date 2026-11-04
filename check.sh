#!/usr/bin/env bash
# This scripts runs various CI-like checks in a convenient way.
# Everything builds in release mode only, so no debug build lands in target/debug.
set -eux

cargo check --quiet --release --workspace --all-targets
cargo check --quiet --release --workspace --all-features --lib --target wasm32-unknown-unknown
cargo fmt --all -- --check
cargo clippy --quiet --release --workspace --all-targets --all-features --  -D warnings -W clippy::all
cargo test --quiet --release --workspace --all-targets --all-features
cargo test --quiet --release --workspace --doc
(cd blip && trunk build --release)
