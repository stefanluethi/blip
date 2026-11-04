# CLAUDE.md

blip is a lightweight web tracing tool for embedded systems. See `README.md` for the
overview and `doc/source` for the documentation: how the parts fit together, a
section per part (blip, outpost, sonde) with its guides and internals, the
reference and the requirements.

| Directory  | Contents |
|------------|----------|
| `blip/`    | GUI app (Rust, egui), web (WASM, Trunk) and native |
| `proto/`   | Wire protocol types shared between blip and the bridge (Rust) |
| `outpost/` | `blip-outpost`, the Python library and examples (uv) |
| `sonde/`   | Tracing library for targets and unit tests (C++17, CMake) |
| `doc/`     | Documentation (Sphinx, sphinx-needs), diagrams in draw.io |

## Release builds only

Build, check, test and run in release mode only, never in debug. A debug build of
the workspace fills `target/debug` with tens of GB. This covers every cargo command
that compiles (`build`, `run`, `check`, `clippy`, `test`, `doc`) and Trunk:

```bash
cargo build --release -p blip
cargo run --release -p blip
cargo check --release --workspace --all-targets
cargo clippy --release --workspace --all-targets --all-features -- -D warnings -W clippy::all
cargo test --release --workspace --all-targets --all-features
cargo test --release --workspace --doc
(cd blip && trunk build --release)
(cd blip && trunk serve --release)
```

For sonde, configure CMake with `-DCMAKE_BUILD_TYPE=Release`:

```bash
cmake -S sonde -B sonde/build -DCMAKE_BUILD_TYPE=Release
cmake --build sonde/build --parallel
ctest --test-dir sonde/build --output-on-failure
```

Cross builds for Cortex-M use `MinSizeRel`, as in `.github/workflows/sonde.yml`.

## Checks

`./check.sh` runs the CI-style checks (all in release): cargo check for native and
WASM, rustfmt, clippy, the tests and a Trunk build. Run it before committing Rust
changes.

- outpost: `cd outpost && uv run pytest`
- docs: `cd doc && make html` (warnings are errors)
- screenshots in the docs: `cd doc && make screenshots`

## Conventions

- Update the docs in `doc/source` (guides, architecture, requirements and tests) along
  with the code they describe.
- Commit messages: a short imperative subject in plain words, then a body that explains
  what changed and why.
