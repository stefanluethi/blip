# Lean code

Every line in this repository is read again by each agent that later works near it, is
built, reviewed and kept in step with the docs. A line that is not needed costs all of
that and gives nothing back. These rules apply to the code and to its tests. `CLAUDE.md`
holds them in short; the `/lean-review` and `/slim` skills and the `reviewer` check
against this file.

Size is measured with `.claude/skills/lean-review/loc.sh` (tokei; code and tests apart,
per part). Lines are the measure, not the goal: see "What is not bloat".

## Before you write

Go down this list and stop at the first step that does the job.

1. **Does it need to exist?** Only what a requirement, the spec or the user's request
   asks for. No option, parameter, mode or helper for a later need.
2. **Is it there already?** Search before you write a helper, a widget part or a test
   fixture: `grep` the crate for the type it takes and the verb in its name, and look at
   the sibling modules (another view, another source, another sink), which very likely
   solved it. The spec names the functions to reuse; a spec without that list for new
   code is not finished.
3. **Does the standard library or a crate we already depend on do it?** An iterator
   adapter, a `derive`, `serde`, an egui call. No new dependency without the user's OK.
4. **Can existing code take it with a parameter or a small generalisation?** Change the
   one place rather than write its sibling. A small change in behaviour of the existing
   code is fine when it lets both share one path: report it.
5. **Then write the least that does it**, in the module where its user lives.

## Rules

- **One copy.** No block of about ten lines or more that also stands elsewhere with other
  names or constants. The second time you need it, share it; do not wait for a third.
  Shared code goes to the lowest layer both users may use (`store` before `widgets`
  before `workbench`).
- **No abstraction with one user.** No trait with one implementation, no wrapper that
  only passes on, no builder for a struct you can write out, no generic with one type,
  no module for one function. The extension points the library documents (`Store`,
  `Source`, `Viewable` and the like) are the exception.
- **Nothing for later.** No field, flag, argument, enum variant or setting that every
  caller sets the same way or nothing reads.
- **Check at the edge, trust inside.** Data is checked where it enters (decoding the
  protocol, loading a file, user input). Inside, no test for a state the types or the
  caller rule out, no fallback for an error that cannot happen, no `Option` or `Result`
  where it is always `Some` or `Ok`.
- **Replace, do not add beside.** When a change makes code obsolete, the same change
  removes it, with its tests and its docs. No old path kept in case, no re-export, no
  alias. A saved file format that a requirement keeps readable is the one exception.
- **Private first.** An item is private or `pub(crate)` unless an application needs it.
  Every public item is a promise that has to be documented, kept and tested.
- **Comments say why.** No comment that repeats the name or the next line, none that
  tells what the change did. A doc comment on a public item is a line or two; more only
  where its user needs it.
- **Nothing fast without a number.** No cache, pool, batching or hand-rolled structure
  unless the baseline shows it is needed.
- **Tests check behaviour, once.** One test per behaviour a requirement names, at the
  lowest layer that shows it. Variants of one behaviour are rows of one table-driven
  test. Set-up shared by three tests is a helper in that `tests.rs`. No test of the
  standard library, of a dependency or of a `derive`; none that repeats an assertion
  another test makes on the same path.
- **Stay in scope.** No renaming, reordering or reformatting of code the change does not
  need. It hides the change and grows the diff.

## Budget

A change says how large it will be before it is built: the spec (or the brief, on the
light path) gives the expected net lines of code and of tests per slice. After the
build, `loc.sh <base>` gives what it was.

- Code more than about half again over the estimate needs a reason in the report, or a
  `/lean-review`.
- More new test lines than new code lines needs a reason (a parser or a protocol has
  one; a view rarely does).
- A change that replaces something should come out near zero or below.
- A fix is small. A fix that grows by a new type or file is a design change: say so.

## Patterns to look for

For reviews and for `/slim`. Each finding names the place, the pattern, the smaller form
and the lines it saves.

| Pattern | Signs |
|---|---|
| **Copy** | The same shape in sibling modules: per view, per source, per sink, per message variant. Functions with the same name and near-same body in several files. A block pasted and changed in two constants. |
| **Parallel paths** | Two ways to do one thing (two drawing paths, two conversions between the same types, a native and a web variant that differ in one call). |
| **One-user abstraction** | A trait, generic, wrapper, builder or config struct with one implementation or one caller. A function called once that only renames another. |
| **Dead** | Not called outside its own tests. `#[allow(dead_code)]`. `pub` items no application, example or doc uses. Variants never built, fields never read, arguments every caller passes alike. |
| **Speculative** | Options and settings nobody sets, feature switches, "for later" hooks. |
| **Defensive** | A check repeated at each layer, a fallback for what cannot happen, errors mapped and mapped again, clamping of values that are in range by construction. |
| **Long-hand** | A loop that is an iterator chain, a `match` that is a `map` or `?`, a hand-written `impl` that is a `derive`, a `Default` written out, getters around public data, state kept that can be computed. |
| **Comments** | Narration, restated names, doc comments longer than the function, history ("now uses", "no longer"). |
| **Tests** | Set-up copied per test, one test per input instead of a table, the same behaviour asserted in the store, the widget and the app, tests of the mock, helpers named alike in several `tests.rs` files (`harness`, `traces`, `drag`). |
| **Docs and examples** | The same explanation on two pages, an example that repeats another with one value changed. |

## What is not bloat

- **Dense is not lean.** Do not trade lines for cleverness: no golfing, no long chained
  expressions, no macro to save a few lines, no names cut short. rustfmt decides the
  layout. If a reader needs longer to understand the shorter form, keep the longer one.
- **Alike is not the same.** Two blocks that look alike but change for different reasons
  stay apart. Merge what must change together.
- **Coverage stays.** A test goes only when another still checks the same behaviour, or
  the code it tested is gone. Merging tests keeps every assertion.
- **The layers stay.** No shortcut through `&dyn Store`, no GUI crate in `store`, no
  heap, exceptions or RTTI in sonde, to save lines.
- **Requirements stay.** Functionality that a `REQ_*` names is not removed to shrink the
  code. If a requirement itself looks like too much, that is a question for the user.
