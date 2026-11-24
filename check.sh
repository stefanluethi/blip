#!/usr/bin/env bash
# This scripts runs various CI-like checks in a convenient way.
# Everything builds in release mode only, so no debug build lands in target/debug.
set -eux

cargo check --quiet --release --workspace --all-targets
cargo check --quiet --release --workspace --all-features --lib --target wasm32-unknown-unknown
# the features of the library: the store alone, natively and for the web, then with the
# widgets; the store needs no GUI crate
cargo check --quiet --release -p blip-kit --no-default-features
cargo check --quiet --release -p blip-kit --no-default-features --target wasm32-unknown-unknown
cargo check --quiet --release -p blip-kit
# the example application built on the library, also for the web
cargo check --quiet --release -p blip-kit --example dashboard --target wasm32-unknown-unknown
if cargo tree -p blip-kit --no-default-features -e normal | grep -E '\b(egui|eframe)'; then
    echo "blip-kit without features depends on a GUI crate" >&2
    exit 1
fi
cargo fmt --all -- --check
cargo clippy --quiet --release --workspace --all-targets --all-features --  -D warnings -W clippy::all
cargo test --quiet --release --workspace --all-targets --all-features
cargo test --quiet --release --workspace --doc
# the API documentation of the crates, without broken links
RUSTDOCFLAGS="-D warnings" cargo doc --quiet --release --no-deps --workspace --all-features
# the library crates as they would be published, not built again: needs the index of
# crates.io
cargo package --quiet --allow-dirty --no-verify -p blip-proto -p blip-kit
(cd blip && trunk build --release)
