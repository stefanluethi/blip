# CLAUDE.md

blip is a lightweight web tracing tool for embedded systems. See `README.md` for the
overview and `doc/source` for the documentation: how the parts fit together, a
section per part (blip, outpost, sonde) with its guides and internals, the
reference and the requirements.

| Directory  | Contents |
|------------|----------|
| `blip/`    | GUI app (Rust, egui), web (WASM, Trunk) and native: `main.rs` and the `App` on `blip-kit` |
| `kit/`   | `blip-kit`, the library blip is built from: `store`, `widgets` and `workbench` as features (Rust) |
| `proto/`   | `blip-proto`, the wire protocol types shared between blip and the bridge (Rust) |
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
cargo run --release -p blip-kit --example dashboard
cargo check --release --workspace --all-targets
cargo clippy --release --workspace --all-targets --all-features -- -D warnings -W clippy::all
cargo test --release --workspace --all-targets --all-features
cargo test --release --workspace --doc
(cd blip && trunk build --release)
(cd blip && trunk serve --release)
```

sonde builds with CMake in `Release`: the commands are in `sonde/CLAUDE.md`.

A hook (`.claude/hooks/release-only.py`, registered in `.claude/settings.json`) blocks
Bash commands that would build in debug. The project's `blip-lsp` plugin
(`.claude/marketplace`) runs rust-analyzer in release mode, and clangd on the compile
database that the sonde configure above writes to `sonde/build`.

cargo calls rustc through `.cargo/rustc-wrapper.sh`, which uses
[sccache](https://github.com/mozilla/sccache) when it is installed: the dependencies
are then compiled once for all checkouts and worktrees, and a new worktree does not
start with a build of many minutes. Before the first build of a session, run
`command -v sccache`. If it prints nothing, warn the user that sccache is not
installed, that builds work but every worktree compiles the dependencies again, and
that `cargo install sccache --locked` installs it; then go on.

The release profile is incremental (`Cargo.toml`): after an edit, cargo recompiles the
changed parts of a workspace crate only. The web profile is not.

## While a change is in work

- Check and test the crate you edit, not the workspace, and name the test:
  `cargo check -q --release -p blip-kit --all-targets --message-format=short`,
  `cargo test -q --release -p blip-kit --lib <filter>`. Run `./check.sh --fast` once,
  when the change is complete.
- Keep the output short: `-q` on every cargo command, and for a long run write to a log
  and read its end: `log=$(mktemp); ./check.sh --fast > "$log" 2>&1; echo "exit=$?"; tail -n 40 "$log"`.
- Use the LSP tool (rust-analyzer) for the diagnostics of a file you edited and to find
  a definition or the references of an item, before `cargo check` and `grep`.
- A hook (`.claude/hooks/rustfmt.py`) formats each Rust file you edit: do not run
  `cargo fmt` yourself.

## Checks

`./check.sh` runs the CI-style checks (all in release): cargo check for native and
WASM, the features of `blip-kit` and its example, rustfmt, clippy, the tests, the API
docs, the packaging of the library crates and a Trunk build. Run it before committing Rust
changes. `./check.sh --fast` stops after the tests: it leaves out the API docs, the
packaging and the Trunk build, and is the check to run when a change is complete.

- outpost: `cd outpost && uv run pytest`
- docs: `cd doc && make html` (warnings are errors)
- screenshots in the docs: `cd doc && make screenshots`

## Conventions

- Update the docs in `doc/source` (guides, architecture, requirements and tests) along
  with the code they describe.
- `kit/CLAUDE.md` has the layer rules of `blip-kit`.
- Rust unit tests live in their own file: `src/x/foo/tests.rs`, declared in `foo.rs` as
  `#[cfg(test)] mod tests;`, not inline, so reading code doesn't load its tests. Skip
  `tests.rs` files when searching or reading code you don't test.
- Commit messages: a short imperative subject in plain words, then a body that explains
  what changed and why.

## Keep the code small

Every line is read again by each agent that works near it. Before you write, search for
what is already there; the full rules and the patterns are in
`.claude/skills/lean-review/RULES.md`.

- Build only what the requirement or the request asks for: no option, parameter or
  helper for later.
- One copy: before a new helper, widget part or test fixture, `grep` for it and look at
  the sibling modules. Extend the existing code with a parameter rather than write its
  sibling.
- No abstraction with one user (trait, wrapper, builder, generic), outside the
  library's documented extension points.
- Check data where it enters; inside, no checks or fallbacks for what cannot happen.
- A change removes what it makes obsolete, with its tests and docs. Nothing is kept
  beside the new way.
- Comments say why, not what. Doc comments on public items are a line or two.
- One test per behaviour; variants are rows of a table-driven test, shared set-up is a
  helper.
- Shorter is not the goal where it is harder to read: no golfing.

`.claude/skills/lean-review/loc.sh [ref]` counts the lines of code and of tests per part
with tokei, against a ref if given. `/lean-review` checks a change for size before it
is committed; `/slim [part]` shrinks existing code in checked batches.

## Agents

`.claude/agents` holds the specialist agents: `designer`, `architect`, `requirements-engineer`,
`blip-dev`, `sonde-dev`, `outpost-dev`, `docs-writer`, `reviewer` and
`design-reviewer`. `/orchestrate <change>` (`.claude/skills/orchestrate`) leads a change
through them: clarify with the user, pick a design for UI changes and an architecture
for changes to an API or the structure, write and approve the requirements, for a large change spec the code on Opus (`.claude/specs`) and implement it in slices on Sonnet,
document, then review until every requirement in scope is implemented, tested, traced
and documented, and the UI looks right. A small change in one part takes a light path:
one specialist and one review on Sonnet.

`design/DESIGN.md` describes the look of blip and the UI rules; follow it for UI work.
